use fst::MapBuilder;
use gannyu_input_core::{Dictionary, Manifest, RegionConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

type BuildResult<T> = Result<T, Box<dyn Error>>;

#[derive(Serialize, Deserialize)]
struct Postings(Vec<Vec<u32>>);

fn compact_key(value: &str) -> String {
    value.trim().replace(' ', "").to_ascii_lowercase()
}

fn remove_if_exists(path: &Path) -> BuildResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

fn clear_generated_resources(region_base: &Path, dictionary_dir: &Path) -> BuildResult<()> {
    for name in [
        "dictionary_runtime_cache.zst",
        "dictionary_runtime_cache.bin",
        "dictionary_runtime_cache.bin.zst",
        "dictionary_runtime_cache.bin.gz",
        "dictionary_runtime_cache.gzp",
    ] {
        remove_if_exists(&dictionary_dir.join(name))?;
    }
    for name in ["fst_map.bin", "postings.bin", "topk.bin"] {
        remove_if_exists(&region_base.join("indexes").join(name))?;
    }
    Ok(())
}

fn write_fst(
    region_id: &str,
    output: &Path,
    keys: &BTreeMap<String, Vec<u32>>,
    frequencies: &[u64],
) -> BuildResult<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let mut postings = Vec::new();
    let mut topk = Vec::new();
    let mut fst_entries = Vec::new();
    for (key, ids) in keys {
        let index = postings.len() as u64;
        postings.push(ids.clone());
        let mut sorted = ids.clone();
        sorted.sort_by(|a, b| {
            let left = frequencies.get(*a as usize).copied().unwrap_or(0);
            let right = frequencies.get(*b as usize).copied().unwrap_or(0);
            right.cmp(&left).then_with(|| a.cmp(b))
        });
        sorted.truncate(8);
        topk.push(sorted);
        fst_entries.push((key.clone(), index));
    }

    std::fs::create_dir_all(output)?;
    let mut postings_file = BufWriter::new(File::create(output.join("postings.bin"))?);
    bincode::serialize_into(&mut postings_file, &Postings(postings))?;
    postings_file.flush()?;
    let mut topk_file = BufWriter::new(File::create(output.join("topk.bin"))?);
    bincode::serialize_into(&mut topk_file, &Postings(topk))?;
    topk_file.flush()?;
    let count = fst_entries.len();
    let mut writer = MapBuilder::new(BufWriter::new(File::create(output.join("fst_map.bin"))?))?;
    for (key, index) in fst_entries {
        writer.insert(key, index)?;
    }
    writer.finish()?;
    println!("gonnyu-resource-build: [{region_id}] wrote {count} keys");
    Ok(())
}

fn included(path: &str) -> bool {
    path == "manifest.toml"
        || path == "fuzzy_scheme.tsv"
        || path.ends_with("/region.toml")
        || path.ends_with("/phonology/syllables.jsonl")
}

fn copy_resources(source: &Path, output: &Path, relative: &Path) -> BuildResult<()> {
    let mut entries = std::fs::read_dir(source.join(relative))?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        let child = relative.join(entry.file_name());
        if kind.is_dir() {
            copy_resources(source, output, &child)?;
            continue;
        }
        let normalized = child.to_string_lossy().replace('\\', "/");
        if !kind.is_file() || !included(&normalized) {
            continue;
        }
        let destination = output.join(&child);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), destination)?;
    }
    Ok(())
}

pub fn build(resources: &Path, output: &Path) -> BuildResult<u32> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    std::fs::create_dir_all(output)?;
    copy_resources(resources, output, Path::new(""))?;

    let manifest: Manifest =
        toml::from_str(&std::fs::read_to_string(resources.join("manifest.toml"))?)?;
    let mut total = 0u32;
    for region_entry in &manifest.regions {
        if region_entry.status != "active" {
            continue;
        }
        let region_path = resources.join(&region_entry.path);
        let region: RegionConfig = toml::from_str(&std::fs::read_to_string(&region_path)?)?;
        let region_base = region_path.parent().ok_or("region path has no parent")?;
        let paths: Vec<PathBuf> = [
            region.language.chars.as_ref(),
            region.language.words.as_ref(),
            region.language.gan_chars.as_ref(),
            region.language.gan_words.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|path| region_base.join(path))
        .collect();
        if paths.is_empty() {
            continue;
        }

        let mut dictionary = Dictionary::load_split_tsvs_uncached(&paths)?;
        let count = dictionary.entries().len();
        eprintln!(
            "gonnyu-resource-build: [{}] loaded {count} entries",
            region_entry.id
        );
        let relative_region = Path::new(&region_entry.path)
            .parent()
            .ok_or("region path has no parent")?;
        let output_region = output.join(relative_region);
        let output_dictionary = output_region.join("dictionaries");
        clear_generated_resources(&output_region, &output_dictionary)?;
        std::fs::create_dir_all(&output_dictionary)?;
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary.write_runtime_cache(output_dictionary.join("dictionary_runtime_cache.zst"))?;

        let mut keys: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        let mut frequencies = Vec::with_capacity(count);
        for (index, entry) in dictionary.entries().iter().enumerate() {
            frequencies.push(entry.frequency.unwrap_or(0));
            for key in [
                compact_key(&entry.dialect_pinyin),
                compact_key(&entry.mandarin_pinyin),
            ] {
                if !key.is_empty() {
                    keys.entry(key).or_default().push(index as u32);
                }
            }
        }
        write_fst(
            &region_entry.id,
            &output_region.join("indexes"),
            &keys,
            &frequencies,
        )?;
        total += count as u32;
    }
    if total == 0 {
        return Err("no dictionary entries found in active regions".into());
    }
    println!("gonnyu-resource-build: wrote {total} entries");
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{compact_key, copy_resources};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "gonnyu-resource-build-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn key_is_compact_and_lowercase() {
        assert_eq!(compact_key(" Gon Nyu "), "gonnyu");
    }

    #[test]
    fn only_runtime_resources_are_copied() {
        let root = root();
        let source = root.join("source");
        let output = root.join("output");
        std::fs::create_dir_all(source.join("frequency")).unwrap();
        std::fs::create_dir_all(source.join("regions/test/phonology")).unwrap();
        std::fs::create_dir_all(source.join("regions/test/dictionaries")).unwrap();
        std::fs::write(source.join("manifest.toml"), "").unwrap();
        std::fs::write(source.join("frequency/base.jsonl"), "").unwrap();
        std::fs::write(source.join("regions/test/region.toml"), "").unwrap();
        std::fs::write(source.join("regions/test/phonology/syllables.jsonl"), "").unwrap();
        std::fs::write(source.join("regions/test/dictionaries/words.tsv"), "").unwrap();
        std::fs::create_dir_all(&output).unwrap();

        copy_resources(&source, &output, Path::new("")).unwrap();

        assert!(output.join("manifest.toml").is_file());
        assert!(!output.join("frequency/base.jsonl").exists());
        assert!(output.join("regions/test/region.toml").is_file());
        assert!(output
            .join("regions/test/phonology/syllables.jsonl")
            .is_file());
        assert!(!output.join("regions/test/dictionaries/words.tsv").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
