//
//  CategoriesBottomView.swift
//  ISEmojiView
//
//  Created by Beniamin Sarkisyan on 01/08/2018.
//

import Foundation
import UIKit

private let MinCellSize = CGFloat(35)

protocol CategoriesBottomViewDelegate: AnyObject {
    func categoriesBottomViewDidSelectCategory(_ category: Category, bottomView: CategoriesBottomView)
    func categoriesBottomViewDidPressChangeKeyboardButton(_ bottomView: CategoriesBottomView)
    func categoriesBottomViewDidPressDeleteBackwardButton(_ bottomView: CategoriesBottomView)
}

final class CategoriesBottomView: UIView {
    // MARK: - Internal variables

    weak var delegate: CategoriesBottomViewDelegate?
    var needToShowAbcButton: Bool? {
        didSet {
            guard let showAbcButton = needToShowAbcButton else {
                return
            }

            changeKeyboardButton.isHidden = !showAbcButton
            collectionViewToSuperViewLeadingConstraint.priority = showAbcButton ? .defaultHigh : .defaultLow
        }
    }

    var categories: [Category]! {
        didSet {
            collectionView.reloadData()

            if let selectedItems = collectionView.indexPathsForSelectedItems, selectedItems.isEmpty {
                selectFirstCell()
            }
        }
    }

    // TaigiKeyboard: local patch — host theme colors for the category icons, "ABC" title
    // and delete icon; nil restores the xib / asset defaults.
    var colors: EmojiViewColors? {
        didSet {
            applyColors()
        }
    }

    // MARK: - IBOutlets

    @IBOutlet private weak var changeKeyboardButton: UIButton!
    @IBOutlet private weak var deleteButton: UIButton! {
        didSet {
            deleteButton.setImage(deleteImage, for: .normal)
        }
    }

    @IBOutlet private weak var collectionView: UICollectionView! {
        didSet {
            collectionView.register(CategoryCell.self, forCellWithReuseIdentifier: "CategoryCell")
            collectionView.hideScrollEdgeEffects() // TaigiKeyboard: local patch
        }
    }

    // TaigiKeyboard: local patch — kept so `applyColors` can restore the unthemed look.
    private let deleteImage = UIImage(named: "ic_emojiDelete", in: Bundle.podBundle, compatibleWith: nil)
    private lazy var defaultAbcTitleColors = (
        normal: changeKeyboardButton.titleColor(for: .normal),
        highlighted: changeKeyboardButton.titleColor(for: .highlighted),
    )

    @IBOutlet private var collectionViewToSuperViewLeadingConstraint: NSLayoutConstraint!

    @IBOutlet private weak var collectionViewToSuperViewTrailingConstraint: NSLayoutConstraint!

    // MARK: - Init functions

    static func loadFromNib(with categories: [Category], needToShowAbcButton: Bool, needToShowDeleteButton: Bool) -> CategoriesBottomView {
        let nibName = String(describing: CategoriesBottomView.self)

        guard let nib = Bundle.podBundle.loadNibNamed(nibName, owner: nil, options: nil) as? [CategoriesBottomView] else {
            fatalError()
        }

        guard let bottomView = nib.first else {
            fatalError()
        }

        bottomView.categories = categories
        bottomView.changeKeyboardButton.isHidden = !needToShowAbcButton
        bottomView.deleteButton.isHidden = !needToShowDeleteButton

        if needToShowAbcButton {
            bottomView.collectionViewToSuperViewLeadingConstraint.priority = .defaultHigh
        }

        if !needToShowDeleteButton {
            bottomView.collectionViewToSuperViewTrailingConstraint.priority = .defaultHigh
        }

        bottomView.selectFirstCell()

        return bottomView
    }

    // MARK: - Override functions

    override func layoutSubviews() {
        super.layoutSubviews()

        if let layout = collectionView.collectionViewLayout as? UICollectionViewFlowLayout {
            var size = collectionView.bounds.size

            if categories.count < Category.count - 2 {
                size.width = MinCellSize
            } else {
                size.width = collectionView.bounds.width / CGFloat(categories.count)
            }

            layout.itemSize = size
            collectionView.collectionViewLayout.invalidateLayout()
        }
    }

    // MARK: - Internal functions

    func updateCurrentCategory(_ category: Category) {
        guard let item = categories.firstIndex(where: { $0 == category }) else {
            return
        }

        guard let selectedItem = collectionView.indexPathsForSelectedItems?.first?.item else {
            return
        }

        guard selectedItem != item else {
            return
        }

        (0 ..< categories.count).forEach {
            let indexPath = IndexPath(item: $0, section: 0)
            collectionView.deselectItem(at: indexPath, animated: false)
        }

        let indexPath = IndexPath(item: item, section: 0)
        collectionView.selectItem(at: indexPath, animated: true, scrollPosition: .centeredHorizontally)
    }

    // MARK: - IBActions

    @IBAction private func changeKeyboard() {
        delegate?.categoriesBottomViewDidPressChangeKeyboardButton(self)
    }

    @IBAction private func deleteBackward() {
        delegate?.categoriesBottomViewDidPressDeleteBackwardButton(self)
    }
}

// MARK: - UICollectionViewDataSource

extension CategoriesBottomView: UICollectionViewDataSource {
    func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
        categories.count
    }

    func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
        let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "CategoryCell", for: indexPath) as! CategoryCell
        cell.setEmojiCategory(categories[indexPath.item])
        cell.colors = colors // TaigiKeyboard: local patch
        return cell
    }
}

// MARK: - UICollectionViewDelegate

extension CategoriesBottomView: UICollectionViewDelegate {
    func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
        delegate?.categoriesBottomViewDidSelectCategory(categories[indexPath.item], bottomView: self)
    }
}

// MARK: - Private functions

extension CategoriesBottomView {
    // TaigiKeyboard: local patch — the category row never scrolls, so the visible cells are all of them.
    private func applyColors() {
        changeKeyboardButton.setTitleColor(colors?.foreground ?? defaultAbcTitleColors.normal, for: .normal)
        changeKeyboardButton.setTitleColor(colors?.dimmedForeground ?? defaultAbcTitleColors.highlighted, for: .highlighted)
        deleteButton.setImage(colors == nil ? deleteImage : deleteImage?.withRenderingMode(.alwaysTemplate), for: .normal)
        deleteButton.tintColor = colors?.foreground
        collectionView.visibleCells.forEach { ($0 as? CategoryCell)?.colors = colors }
    }

    private func selectFirstCell() {
        let indexPath = IndexPath(item: 0, section: 0)
        collectionView.selectItem(at: indexPath, animated: true, scrollPosition: .centeredHorizontally)
    }
}
