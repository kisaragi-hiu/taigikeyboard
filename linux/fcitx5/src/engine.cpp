/*
 * See engine.h. Every framework call here has its origin in fcitx5-rime's
 * rimestate.cpp / rimeengine.cpp (the reference addon); the decisions are
 * the Rust core's.
 */
#include "engine.h"

#include <fcitx-utils/i18n.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/utf8.h>
#include <fcitx/candidatelist.h>
#include <fcitx/event.h>
#include <fcitx/inputpanel.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h>
#include <memory>
#include <string>
#include <vector>

namespace fcitx::taigi {

FCITX_DEFINE_LOG_CATEGORY(taigi_log, "taigikeyboard");
#define TAIGI_DEBUG() FCITX_LOGC(taigi_log, Debug)
#define TAIGI_ERROR() FCITX_LOGC(taigi_log, Error)

namespace {

/* The Fcitx5 KeyState bits are the X11 / IBus ones the Rust core reads
 * (Shift 1<<0, Ctrl 1<<2, Alt/Mod1 1<<3, Mod4 1<<6, Super 1<<26); a release
 * is a separate flag on the event, added here as the IBus release bit
 * (fcitx5-rime does the same: `intStates |= (1 << 30)`). */
uint32_t statesFor(const KeyEvent &event) {
    uint32_t states = static_cast<uint32_t>(event.rawKey().states());
    if (event.isRelease()) {
        states |= TAIGI_STATE_RELEASE;
    }
    return states;
}

/* A candidate the panel shows: selecting it (a click) only highlights, as
 * on macOS and Windows; the commit stays a key's. */
class CandidateWordImpl final : public CandidateWord {
public:
    CandidateWordImpl(State *state, uint32_t position, Text text)
        : CandidateWord(std::move(text)), state_(state), position_(position) {}

    void select(InputContext * /*ic*/) const override { state_->click(position_); }

private:
    State *state_;
    uint32_t position_;
};

} // namespace

// ---------------------------------------------------------------------------
// State — one input context
// ---------------------------------------------------------------------------

State::State(Engine *engine, InputContext &ic) : engine_(engine), ic_(ic) {
    handle_ = taigi_engine_new(engine_->runtime());
    if (!handle_) {
        TAIGI_ERROR() << "taigi_engine_new answered null";
    }
    syncCapabilities();
}

State::~State() { taigi_engine_free(handle_); }

void State::syncCapabilities() {
    if (!handle_) {
        return;
    }
    uint32_t caps = 0;
    if (ic_.capabilityFlags().test(CapabilityFlag::SurroundingText)) {
        caps |= TAIGI_CAP_SURROUNDING_TEXT;
    }
    taigi_engine_set_capabilities(handle_, caps);
}

void State::keyEvent(KeyEvent &event) {
    if (!handle_) {
        return;
    }
    TaigiReply *reply = taigi_engine_key(handle_, event.rawKey().sym(), event.rawKey().code(),
                                         statesFor(event));
    if (!reply) {
        return;
    }
    const bool handled = taigi_reply_handled(reply);
    replay(reply);
    if (handled) {
        event.filterAndAccept();
    }
}

void State::endSession() {
    if (!handle_) {
        return;
    }
    replay(taigi_engine_end_session(handle_));
}

void State::navigate(uint32_t direction) {
    if (!handle_) {
        return;
    }
    replay(taigi_engine_navigate(handle_, direction));
}

void State::click(uint32_t position) {
    if (!handle_) {
        return;
    }
    replay(taigi_engine_click(handle_, position));
}

/* The reply, entry by entry, onto the input context; one UI update at the
 * end (fcitx5-rime `updateUI`). */
void State::replay(TaigiReply *reply) {
    if (!reply) {
        return;
    }
    auto &panel = ic_.inputPanel();
    bool preeditChanged = false;
    bool panelChanged = false;
    const size_t count = taigi_reply_count(reply);
    for (size_t i = 0; i < count; ++i) {
        switch (taigi_reply_kind(reply, i)) {
        case TAIGI_EMIT_PREEDIT: {
            Text preedit(taigi_reply_text(reply, i), TextFormatFlag::Underline);
            preedit.setCursor(static_cast<int>(
                utf8::ncharByteLength(preedit.toString().begin(), taigi_reply_caret(reply, i))));
            if (ic_.capabilityFlags().test(CapabilityFlag::Preedit)) {
                panel.setClientPreedit(preedit);
                panel.setPreedit(Text());
            } else {
                /* A client that cannot draw a preedit (a terminal): the panel
                 * shows it above the candidates instead. */
                panel.setClientPreedit(Text());
                panel.setPreedit(preedit);
            }
            preeditChanged = true;
            panelChanged = true;
            break;
        }
        case TAIGI_EMIT_CLEAR_PREEDIT:
            panel.setClientPreedit(Text());
            panel.setPreedit(Text());
            preeditChanged = true;
            panelChanged = true;
            break;
        case TAIGI_EMIT_COMMIT:
            ic_.commitString(taigi_reply_text(reply, i));
            break;
        case TAIGI_EMIT_DELETE_SURROUNDING:
            ic_.deleteSurroundingText(taigi_reply_delete_offset(reply, i),
                                      taigi_reply_delete_count(reply, i));
            break;
        case TAIGI_EMIT_LOOKUP_TABLE:
            showCandidates(reply, i);
            panelChanged = true;
            break;
        case TAIGI_EMIT_HIDE_LOOKUP_TABLE:
            panel.setCandidateList(nullptr);
            panelChanged = true;
            break;
        default:
            TAIGI_ERROR() << "unknown reply kind " << taigi_reply_kind(reply, i);
            break;
        }
    }
    taigi_reply_free(reply);
    if (preeditChanged) {
        ic_.updatePreedit();
    }
    if (panelChanged) {
        ic_.updateUserInterface(UserInterfaceComponent::InputPanel);
    }
}

/* The list as the panel draws it: page size = the slot keys, labels = the
 * slot keys, the highlight at the absolute index the core keeps. Paging and
 * cursor moves from the panel go back through the core (`navigate`), so
 * the list is rebuilt from the next reply rather than paged locally. */
void State::showCandidates(const TaigiReply *reply, size_t index) {
    auto list = std::make_unique<CommonCandidateList>();
    const size_t rows = taigi_reply_table_count(reply, index);
    const size_t labels = taigi_reply_table_label_count(reply, index);
    const uint32_t pageSize = taigi_reply_table_page_size(reply, index);
    std::vector<std::string> labelTexts;
    labelTexts.reserve(labels);
    for (size_t position = 0; position < labels; ++position) {
        labelTexts.emplace_back(std::string(taigi_reply_table_label(reply, index, position)) + " ");
    }
    list->setLabels(labelTexts);
    list->setPageSize(static_cast<int>(pageSize));
    list->setLayoutHint(taigi_reply_table_vertical(reply, index) ? CandidateLayoutHint::Vertical
                                                                  : CandidateLayoutHint::Horizontal);
    /* The panel's own selection keys are switched off: a slot key reaches
     * the core through keyEvent, which is what keeps Shift+slot (the other
     * script) and the Digits set working the same way on every platform. */
    list->setSelectionKey(KeyList());
    for (size_t row = 0; row < rows; ++row) {
        list->append(std::make_unique<CandidateWordImpl>(
            this, static_cast<uint32_t>(row % (pageSize == 0 ? 1 : pageSize)),
            Text(taigi_reply_table_candidate(reply, index, row))));
    }
    list->setGlobalCursorIndex(static_cast<int>(taigi_reply_table_cursor(reply, index)));
    ic_.inputPanel().setCandidateList(std::move(list));
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

Engine::Engine(Instance *instance)
    : instance_(instance),
      factory_([this](InputContext &ic) { return new State(this, ic); }) {
    runtime_ = taigi_runtime_new();
    if (!runtime_) {
        TAIGI_ERROR() << "taigi_runtime_new answered null; the engine will type nothing";
    }
    TAIGI_DEBUG() << "taigikeyboard " << taigi_version() << " loaded";
    instance_->inputContextManager().registerProperty("taigikeyboardState", &factory_);
}

Engine::~Engine() {
    /* Every state (and its engine handle) goes with the property registration;
     * the runtime is freed last, as the header requires. */
    factory_.unregister();
    taigi_runtime_free(runtime_);
}

void Engine::activate(const InputMethodEntry & /*entry*/, InputContextEvent &event) {
    if (auto *s = state(event.inputContext())) {
        s->syncCapabilities();
    }
}

void Engine::deactivate(const InputMethodEntry &entry, InputContextEvent &event) {
    /* Switching away commits nothing extra: the framework has already
     * committed the client preedit (the platform-wide focus-loss rule, L4);
     * the engine forgets the composition. */
    reset(entry, event);
}

void Engine::keyEvent(const InputMethodEntry & /*entry*/, KeyEvent &keyEvent) {
    if (auto *s = state(keyEvent.inputContext())) {
        s->keyEvent(keyEvent);
    }
}

void Engine::reset(const InputMethodEntry & /*entry*/, InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (auto *s = state(ic)) {
        s->endSession();
    }
    ic->inputPanel().reset();
    ic->updatePreedit();
    ic->updateUserInterface(UserInterfaceComponent::InputPanel);
}

std::string Engine::subModeLabelImpl(const InputMethodEntry & /*entry*/, InputContext & /*ic*/) {
    /* The mode letter the panel shows beside the icon; the romanization /
     * script state follows in the chrome PR (roadmap L6). */
    return "台";
}

AddonInstance *EngineFactory::create(AddonManager *manager) {
    return new Engine(manager->instance());
}

} // namespace fcitx::taigi

FCITX_ADDON_FACTORY(fcitx::taigi::EngineFactory)
