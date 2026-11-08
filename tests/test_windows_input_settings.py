from pathlib import Path
import shutil
import subprocess

import pytest


ROOT = Path(__file__).resolve().parents[1]


def test_region_and_settings_menus_dispatch_independently(tmp_path: Path) -> None:
    compiler = next((path for name in ("clang++", "g++", "c++") if (path := shutil.which(name))), None)
    if compiler is None:
        pytest.skip("requires a C++ compiler")
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text()
    start = source.index("STDMETHODIMP GannyuMenuButton::InitMenu(")
    end = source.index("STDMETHODIMP GannyuMenuButton::GetIcon(", start)
    harness = r'''
#include <array>
#include <cassert>
#include <cwchar>
#include <string>
#include <vector>
#define STDMETHODIMP int
using UINT = unsigned int;
using DWORD = unsigned long;
using ULONG = unsigned long;
constexpr int S_OK = 0, E_INVALIDARG = -1;
constexpr DWORD TF_LBMENUF_CHECKED = 1, TF_LBMENUF_GRAYED = 2;
struct MenuItem { UINT id; DWORD flags; std::wstring label; };
struct ITfMenu {
    std::vector<MenuItem> items;
    int AddMenuItem(UINT id, DWORD flags, void*, void*, const wchar_t* text, ULONG length, void*) {
        items.push_back({id, flags, std::wstring(text, length)});
        return S_OK;
    }
};
struct GannyuTextService {
    std::vector<std::string> regions{"lancong", "fenni"};
    std::array<int, 9> options{{1, 1, 1, 1, 1, 0, 1, 1, 0}};
    std::string selected = "lancong";
    UINT option = 0;
    bool scheme_available = true;
    const auto& RegionIds() const { return regions; }
    std::wstring RegionLabel(size_t index) const { return index == 0 ? L"南昌" : L"分宜"; }
    const auto& CurrentRegionId() const { return selected; }
    int InputOption(size_t index) const { return options[index]; }
    bool SchemeAvailable() const { return scheme_available; }
    void SelectInputOption(UINT id) { option = id; }
    void SwitchRegion(const std::string& id) { selected = id; }
};
struct GannyuMenuButton {
    GannyuTextService* owner_;
    bool settings_;
    std::vector<std::string> menuRegionIds_;
    int InitMenu(ITfMenu* menu);
    int OnMenuSelect(UINT id);
};
'''
    checks = r'''
int main() {
    GannyuTextService owner;
    GannyuMenuButton region{&owner, false, {}}, settings{&owner, true, {}};
    ITfMenu regions;
    assert(region.InitMenu(&regions) == S_OK);
    assert(regions.items.size() == 2);
    assert(regions.items[0].label == L"南昌");
    assert(regions.items[0].flags & TF_LBMENUF_CHECKED);
    assert(!(regions.items[1].flags & TF_LBMENUF_CHECKED));
    region.OnMenuSelect(1);
    assert(owner.selected == "fenni");
    region.OnMenuSelect(1000);
    assert(owner.option == 0);
    const wchar_t *labels[] = {L"副标题", L"赣普词语连带", L"同义词连带", L"模糊匹配", L"　普拼兼容", L"　文白匹配", L"　其他模糊规则", L"新老兼有", L"新派", L"老派", L"一式（汉拼式）", L"二式（组合式）"};
    for (int annotations : {0, 1}) {
        for (int fuzzy : {0, 1}) {
            for (int mode = 0; mode < 3; ++mode) {
                for (int second : {0, 1}) {
                    for (int available : {0, 1}) {
                        owner.options = {{annotations, 1, fuzzy, 1, 1, mode, 1, 1, second}};
                        owner.scheme_available = available != 0;
                        ITfMenu options;
                        assert(settings.InitMenu(&options) == S_OK);
                        assert(options.items.size() == 12);
                        for (int i = 0; i < 12; ++i) assert(options.items[i].label == labels[i]);
                        assert(bool(options.items[0].flags & TF_LBMENUF_CHECKED) == (annotations != 0));
                        assert(options.items[1].flags & TF_LBMENUF_CHECKED);
                        assert(options.items[2].flags & TF_LBMENUF_CHECKED);
                        assert(bool(options.items[3].flags & TF_LBMENUF_CHECKED) == (fuzzy != 0));
                        assert(options.items[4].flags & TF_LBMENUF_CHECKED);
                        assert(options.items[5].flags & TF_LBMENUF_CHECKED);
                        assert(options.items[6].flags & TF_LBMENUF_CHECKED);
                        for (int selected = 0; selected < 3; ++selected)
                            assert(bool(options.items[7 + selected].flags & TF_LBMENUF_CHECKED) == (selected == mode));
                        assert(bool(options.items[10].flags & TF_LBMENUF_CHECKED) == (second == 0));
                        assert(bool(options.items[11].flags & TF_LBMENUF_CHECKED) == (second != 0));
                        assert(bool(options.items[3].flags & TF_LBMENUF_GRAYED) == (second != 0));
                        assert(bool(options.items[4].flags & TF_LBMENUF_GRAYED) == (!second && !fuzzy));
                        assert(bool(options.items[5].flags & TF_LBMENUF_GRAYED) == (!second && !fuzzy));
                        assert(bool(options.items[6].flags & TF_LBMENUF_GRAYED) == (second || (!second && !fuzzy)));
                        assert(bool(options.items[11].flags & TF_LBMENUF_GRAYED) == !available);
                    }
                }
            }
        }
    }
    settings.OnMenuSelect(1002);
    assert(owner.option == 1002);
    settings.OnMenuSelect(0);
    assert(owner.selected == "fenni");
    owner.regions.resize(1);
    ITfMenu single_region_settings;
    assert(settings.InitMenu(&single_region_settings) == S_OK);
    assert(single_region_settings.items.size() == 12);
    assert(settings.InitMenu(nullptr) == E_INVALIDARG);
}
'''
    path = tmp_path / "menu_probe.cpp"
    binary = tmp_path / "menu_probe"
    path.write_text(harness + source[start:end] + checks)
    subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", str(path), "-o", str(binary)], check=True, capture_output=True, text=True)
    subprocess.run([str(binary)], check=True, capture_output=True, text=True)
