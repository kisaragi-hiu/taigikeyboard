/*
 * The Fcitx5 shell of Taigi Keyboard: an InputMethodEngineV3 that hands
 * every key to the Rust core through the C ABI (taigikeyboard.h) and
 * replays the reply into the input context — client preedit, commit,
 * delete-surrounding, candidate list. It composes nothing itself: the same
 * `Emit` list drives the IBus shell, so a behaviour that differs between
 * the two is a shell bug (docs/architecture/linux-roadmap.md L1, 2026-09-23).
 *
 * Shape follows fcitx5-rime (src/rimeengine.h, src/rimestate.cpp): one
 * engine, one per-input-context property holding the Rust engine handle.
 */
#ifndef TAIGIKEYBOARD_FCITX5_ENGINE_H
#define TAIGIKEYBOARD_FCITX5_ENGINE_H

#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <memory>
#include <string>

#include "taigikeyboard.h"

namespace fcitx::taigi {

class Engine;

/* The Rust engine handle for one input context, owned by the context
 * (InputContextProperty: created on first use, destroyed with the context). */
class State final : public InputContextProperty {
public:
    State(Engine *engine, InputContext &ic);
    ~State() override;

    void keyEvent(KeyEvent &event);
    void endSession();
    void navigate(uint32_t direction);
    void click(uint32_t position);
    void syncCapabilities();

private:
    void replay(TaigiReply *reply);
    void showCandidates(const TaigiReply *reply, size_t index);

    Engine *engine_;
    InputContext &ic_;
    ::TaigiEngine *handle_ = nullptr;
};

class Engine final : public InputMethodEngineV3 {
public:
    explicit Engine(Instance *instance);
    ~Engine() override;

    Instance *instance() { return instance_; }
    ::TaigiRuntime *runtime() { return runtime_; }

    void activate(const InputMethodEntry &entry, InputContextEvent &event) override;
    void deactivate(const InputMethodEntry &entry, InputContextEvent &event) override;
    void keyEvent(const InputMethodEntry &entry, KeyEvent &keyEvent) override;
    void reset(const InputMethodEntry &entry, InputContextEvent &event) override;
    std::string subModeLabelImpl(const InputMethodEntry &entry, InputContext &ic) override;

    State *state(InputContext *ic) { return ic->propertyFor(&factory_); }

private:
    Instance *instance_;
    ::TaigiRuntime *runtime_ = nullptr;
    FactoryFor<State> factory_;
};

class EngineFactory final : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override;
};

} // namespace fcitx::taigi

#endif
