import UIKit

final class RegionSettingsViewController: UITableViewController {
    private let store = GannyuAppleRegionStore()
    private var regions: [GannyuAppleRegion] = []
    private var selectedID: String?

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "赣语输入法"
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "region")
        reloadRegions()
    }

    private func reloadRegions() {
        do {
            regions = try GannyuAppleEngine.regions()
            selectedID = store.currentID(in: regions)
        } catch {
            regions = []
            selectedID = nil
        }
        tableView.reloadData()
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        regions.count
    }

    override func tableView(
        _ tableView: UITableView,
        cellForRowAt indexPath: IndexPath
    ) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "region", for: indexPath)
        let region = regions[indexPath.row]
        cell.textLabel?.text = region.nameZh
        cell.accessoryType = region.id == selectedID ? .checkmark : .none
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        let region = regions[indexPath.row]
        if store.select(region.id, in: regions) {
            selectedID = region.id
            tableView.reloadData()
        }
        tableView.deselectRow(at: indexPath, animated: true)
    }
}
