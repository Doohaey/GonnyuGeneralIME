from pathlib import Path
from zipfile import ZipFile

from platforms.rime import package_region


def write(path: Path, content: str = "x") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def test_package_region_splits_one_and_two_scheme_archives(tmp_path: Path) -> None:
    output = tmp_path / "output"
    region = "lancong"
    for name in (
        f"gannyu_{region}.schema.yaml",
        f"gannyu_{region}.dict.yaml",
        f"gannyu_{region}_new_rel.dict.yaml",
        f"gannyu_{region}_ipa.schema.yaml",
        f"gannyu_{region}_ipa_gan.schema.yaml",
        f"lua/gannyu_{region}_data.lua",
        f"lua/gannyu_{region}_new_rel_data.lua",
        f"lua/gannyu_{region}_annotations.bin",
        f"lua/gannyu_{region}_new_rel_annotations.bin",
        f"lua/gannyu_{region}_scheme2.lua",
        "lua/gannyu_input_options.lua",
        "lua/gannyu_options_translator.lua",
        "lua/gannyu_annotation_store.lua",
        "lua/gannyu_data_lifecycle.lua",
        "lua/gannyu_annotation_filter.lua",
        "lua/gannyu_single_char_filter.lua",
        "lua/gannyu_relation_filter.lua",
        "LICENSE",
    ):
        write(output / name)

    archives = tmp_path / "archives"
    one = package_region._package(output, archives, region, "universal", scheme2=False)
    two = package_region._package(output, archives, region, "universal", scheme2=True)

    assert one.name.endswith(f"-rime-{region}.zip")
    assert two.name.endswith(f"-rime-{region}-ipa.zip")
    with ZipFile(one) as archive:
        names = set(archive.namelist())
        assert f"gannyu_{region}.schema.yaml" in names
        assert f"gannyu_{region}_ipa.schema.yaml" not in names
        assert f"lua/gannyu_{region}_scheme2.lua" not in names
        assert b"schema: gannyu_lancong\n" in archive.read("default.custom.yaml")
    with ZipFile(two) as archive:
        names = set(archive.namelist())
        assert f"gannyu_{region}_ipa.schema.yaml" in names
        assert f"gannyu_{region}_ipa_gan.schema.yaml" in names
        assert f"gannyu_{region}.schema.yaml" not in names
        assert f"lua/gannyu_{region}_scheme2.lua" in names
        assert f"gannyu_{region}.dict.yaml" in names
        assert f"lua/gannyu_{region}_new_rel_data.lua" in names
        assert b"schema: gannyu_lancong_ipa\n" in archive.read("default.custom.yaml")
