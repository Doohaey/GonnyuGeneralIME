import UIKit

final class RegionSettingsViewController: UITableViewController {
    private enum Section: Int, CaseIterable {
        case setup
        case regions
        case userData
        case help
    }

    private let store = GonnyuAppleRegionStore()
    private var regions: [GonnyuAppleRegion] = []
    private var selectedID: String?
    private var loadError: String?

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "GonnyuInputMethod"
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "cell")
        reloadRegions()
    }

    private func reloadRegions() {
        do {
            regions = try GonnyuAppleEngine.regions()
            selectedID = store.currentID(in: regions)
            loadError = nil
        } catch {
            regions = []
            selectedID = nil
            loadError = "地区资源加载失败，请重新安装输入法。"
        }
        tableView.reloadData()
    }

    override func numberOfSections(in tableView: UITableView) -> Int {
        Section.allCases.count
    }

    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        switch Section(rawValue: section) {
        case .setup: return "完成以下设置"
        case .regions: return "地区方案"
        case .userData: return "用户词库"
        case .help: return "帮助"
        case nil: return nil
        }
    }

    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        switch Section(rawValue: section) {
        case .setup:
            return "完成后即可在任意 App 的输入框使用 Gonnyu。"
        case .regions:
            return loadError
        case .userData:
            return "用户词库仅在本机离线存储。"
        case .help:
            return nil
        case nil:
            return nil
        }
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        switch Section(rawValue: section) {
        case .setup: return 3
        case .regions: return regions.count
        case .userData: return 2
        case .help: return 1
        case nil: return 0
        }
    }

    override func tableView(
        _ tableView: UITableView,
        cellForRowAt indexPath: IndexPath
    ) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "cell", for: indexPath)
        var content = cell.defaultContentConfiguration()
        cell.accessoryType = .none
        switch Section(rawValue: indexPath.section) {
        case .setup:
            if indexPath.row == 0 {
                content.text = "1. 添加 Gonnyu 键盘"
                content.secondaryText = "设置 > 通用 > 键盘 > 键盘 > 添加新键盘…"
            } else if indexPath.row == 1 {
                content.text = "2. 开启允许完全访问"
                content.secondaryText = "让用户词库与词频能保存在本机"
            } else {
                content.text = "3. 切换至 Gonnyu"
                content.secondaryText = "在任意输入框点按或长按 🌐 选择 Gonnyu"
            }
            cell.accessoryType = .disclosureIndicator
        case .regions:
            let region = regions[indexPath.row]
            content.text = region.nameZh
            content.secondaryText = nil
            cell.accessoryType = region.id == selectedID ? .checkmark : .none
        case .userData:
            content.text = ["清空当前地区学习数据", "清空全部地区学习数据"][indexPath.row]
            content.textProperties.color = .systemRed
        case .help:
            content.text = "使用教程"
            content.secondaryText = "拼音、词语标记与输入说明"
            cell.accessoryType = .disclosureIndicator
        case nil:
            break
        }
        cell.contentConfiguration = content
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        defer { tableView.deselectRow(at: indexPath, animated: true) }
        switch Section(rawValue: indexPath.section) {
        case .setup:
            if indexPath.row == 1 {
                openAppSettings()
            } else {
                showSetupInstructions(step: indexPath.row)
            }
        case .regions:
            let region = regions[indexPath.row]
            if store.select(region.id, in: regions) {
                selectedID = region.id
                tableView.reloadSections(IndexSet(integer: Section.regions.rawValue), with: .automatic)
            }
        case .userData:
            confirmClear(allRegions: indexPath.row == 1)
        case .help:
            navigationController?.pushViewController(TutorialViewController(), animated: true)
        case nil:
            break
        }
    }

    private func showSetupInstructions(step: Int) {
        let title: String
        let message: String
        if step == 0 {
            title = "添加 Gonnyu 键盘"
            message = "请依次打开：\n设置 > 通用 > 键盘 > 键盘 > 添加新键盘…\n\n选择 Gonnyu 后返回此 App。"
        } else {
            title = "切换至 Gonnyu"
            message = "打开任意可输入文字的 App，点按或长按键盘左下角的 🌐，然后选择 Gonnyu。"
        }
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "知道了", style: .default))
        present(alert, animated: true)
    }

    private func openAppSettings() {
        guard let settingsURL = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(settingsURL)
    }

    private func confirmClear(allRegions: Bool) {
        let title = allRegions ? "清空全部地区学习数据" : "清空当前地区学习数据"
        let message = "会删除学习得到的用户词与候选排序，且无法恢复。"
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "取消", style: .cancel))
        alert.addAction(UIAlertAction(title: "清空", style: .destructive) { [weak self] _ in
            self?.requestUserDataReset(allRegions: allRegions)
        })
        present(alert, animated: true)
    }

    private func requestUserDataReset(allRegions: Bool) {
        let regionIDs = allRegions ? regions.map(\.id) : [selectedID].compactMap { $0 }
        guard !regionIDs.isEmpty else { return }
        do {
            try store.requestUserDataReset(regionIDs: regionIDs, in: regions)
            let alert = UIAlertController(
                title: "已安排清空",
                message: "请切换到赣语键盘一次以完成操作。",
                preferredStyle: .alert
            )
            alert.addAction(UIAlertAction(title: "好", style: .default))
            present(alert, animated: true)
        } catch {
            let alert = UIAlertController(
                title: "清空失败",
                message: "用户数据未被修改，请稍后重试。",
                preferredStyle: .alert
            )
            alert.addAction(UIAlertAction(title: "好", style: .default))
            present(alert, animated: true)
        }
    }
}
