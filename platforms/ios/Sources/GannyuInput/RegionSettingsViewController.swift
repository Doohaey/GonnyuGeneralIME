import UIKit

final class RegionSettingsViewController: UITableViewController {
    private enum Section: Int, CaseIterable {
        case regions
        case userData
        case setup
    }

    private let store = GannyuAppleRegionStore()
    private var regions: [GannyuAppleRegion] = []
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
            regions = try GannyuAppleEngine.regions()
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
        case .regions: return "地区方案"
        case .userData: return "用户词库"
        case .setup: return "使用方式"
        case nil: return nil
        }
    }

    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        switch Section(rawValue: section) {
        case .regions:
            return loadError ?? "切换地区后，键盘只加载所选地区的资源。"
        case .userData:
            return "学习词与词频保存在本机 App Group，不会上传网络。"
        case .setup:
            return "在系统设置中添加 Gonnyu 键盘后，可用地球键切换到本输入法。"
        case nil:
            return nil
        }
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        switch Section(rawValue: section) {
        case .regions: return regions.count
        case .userData: return 3
        case .setup: return 1
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
        case .regions:
            let region = regions[indexPath.row]
            content.text = region.nameZh
            content.secondaryText = region.id
            cell.accessoryType = region.id == selectedID ? .checkmark : .none
        case .userData:
            content.text = ["清空用户词", "清空学习词频", "清空全部用户数据"][indexPath.row]
            content.textProperties.color = .systemRed
        case .setup:
            content.text = "打开本 App 设置"
            content.secondaryText = "查看键盘启用说明"
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
        case .regions:
            let region = regions[indexPath.row]
            if store.select(region.id, in: regions) {
                selectedID = region.id
                tableView.reloadSections(IndexSet(integer: Section.regions.rawValue), with: .automatic)
            }
        case .userData:
            confirmClear(scope: [
                .words,
                .frequencies,
                .all,
            ][indexPath.row])
        case .setup:
            guard let settingsURL = URL(string: UIApplication.openSettingsURLString) else { return }
            UIApplication.shared.open(settingsURL)
        case nil:
            break
        }
    }

    private func confirmClear(scope: GannyuAppleUserDataScope) {
        let title: String
        let message: String
        switch scope {
        case .words:
            title = "清空用户词"
            message = "这会删除你学习得到的用户词，且无法恢复。"
        case .frequencies:
            title = "清空学习词频"
            message = "这会重置候选词的学习排序，且无法恢复。"
        case .all:
            title = "清空全部用户数据"
            message = "这会删除用户词和学习词频，且无法恢复。"
        }
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "取消", style: .cancel))
        alert.addAction(UIAlertAction(title: "清空", style: .destructive) { [weak self] _ in
            self?.clearUserData(scope)
        })
        present(alert, animated: true)
    }

    private func clearUserData(_ scope: GannyuAppleUserDataScope) {
        guard let regionID = selectedID else { return }
        do {
            let engine = try GannyuAppleEngine(
                regionID: regionID,
                userDataDirectory: store.userDataDirectory
            )
            try engine.clearUserData(scope)
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
