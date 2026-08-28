use gannyu_input_core::{
    default_region_entry, list_region_entries, load_region_from_manifest, CandidateSource,
    CandidateTier, ComposedCandidate, Dictionary, FuzzyMap, InputPipeline, MandarinHintBook,
    NormalizedSyllable, PriorityTier, PronunciationBook, RankedCandidate, RegionResource, Register,
    RetrievalLayer, SlangBook, SyllableScheme, TriggerKind,
};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod forward;

const USAGE: &str = "用法:\n  gannyu-input-cli regions list\n  gannyu-input-cli regions use <region_id>\n  gannyu-input-cli syllable normalize <input> [--scheme gon-han|gon-pin]\n  gannyu-input-cli tone classes [--region <id>]\n  gannyu-input-cli checked alternatives <syllable> [--region <id>]\n  gannyu-input-cli mandarin hint <term> [--region <id>]\n  gannyu-input-cli register check <grapheme> <syllable> [--region <id>]\n  gannyu-input-cli lookup <text> [--region <id>]\n  gannyu-input-cli pipeline compose <input> [--region <id>]\n  gannyu-input-cli pipeline retrieve <input> [--region <id>]\n  gannyu-input-cli forward             检测设备→安装→键盘转发到安卓\n  gannyu-input-cli forward --skip-install  跳过安装，直接开始键盘转发\n";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("错误: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), Box<dyn Error>> {
    let manifest_path = PathBuf::from("resources/manifest.toml");
    match args.first().map(String::as_str) {
        Some("regions") => regions(&manifest_path, &args[1..]),
        Some("syllable") => syllable(&manifest_path, &args[1..]),
        Some("tone") => tone(&manifest_path, &args[1..]),
        Some("checked") => checked(&manifest_path, &args[1..]),
        Some("mandarin") => mandarin(&manifest_path, &args[1..]),
        Some("register") => register(&manifest_path, &args[1..]),
        Some("lookup") => lookup(&manifest_path, &args[1..]),
        Some("pipeline") => pipeline(&manifest_path, &args[1..]),
        Some("forward") => {
            let skip_install = args.get(1).map(String::as_str) == Some("--skip-install")
                || args.get(1).map(String::as_str) == Some("-S");
            forward::run(!skip_install);
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(unknown) => Err(format!("未知子命令: {unknown}\n{USAGE}").into()),
    }
}

fn regions(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("list") | None => {
            let default = default_region_entry(manifest_path)?;
            for region in list_region_entries(manifest_path)? {
                let mark = if region.id == default.id { "*" } else { " " };
                println!(
                    "{mark} {}\t{}\t{}",
                    region.id, region.name_zh, region.status
                );
            }
            Ok(())
        }
        Some("use") => {
            let region_id = rest.get(1).ok_or("regions use 缺少 region_id")?;
            let resource = load_region_from_manifest(manifest_path, region_id)?;
            println!(
                "{}\t{}\t{}",
                resource.config.region.id,
                resource.config.region.name_zh,
                resource.config.region.status
            );
            Ok(())
        }
        Some(unknown) => Err(format!("未知 regions 子命令: {unknown}").into()),
    }
}

fn syllable(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("normalize") => {
            let input = rest.get(1).ok_or("syllable normalize 缺少输入串")?;
            let scheme = parse_scheme_flag(&rest[2..])?;
            let fuzzy_path = resolve_fuzzy_map_path(manifest_path)?;
            let fuzzy = FuzzyMap::load_tsv(&fuzzy_path)?;
            let outputs = fuzzy.normalize(input, scheme);
            print_normalized(input, scheme, &outputs);
            Ok(())
        }
        Some(unknown) => Err(format!("未知 syllable 子命令: {unknown}").into()),
        None => Err("syllable 缺少子命令；可用：normalize".into()),
    }
}

fn parse_scheme_flag(args: &[String]) -> Result<SyllableScheme, Box<dyn Error>> {
    let mut scheme = SyllableScheme::GonPin;
    let mut index = 0;
    while index < args.len() {
        let token = args[index].as_str();
        if token == "--scheme" {
            let value = args.get(index + 1).ok_or("--scheme 缺少参数值")?;
            scheme =
                SyllableScheme::parse(value).ok_or_else(|| format!("无法识别方案: {value}"))?;
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--scheme=") {
            scheme =
                SyllableScheme::parse(value).ok_or_else(|| format!("无法识别方案: {value}"))?;
            index += 1;
            continue;
        }
        return Err(format!("未知参数: {token}").into());
    }
    Ok(scheme)
}

fn resolve_fuzzy_map_path(manifest_path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let path = root.join("fuzzy_scheme.tsv");
    if !path.is_file() {
        return Err(format!("找不到模糊音方案表: {}", path.display()).into());
    }
    Ok(path)
}

fn print_normalized(input: &str, scheme: SyllableScheme, outputs: &[NormalizedSyllable]) {
    println!("输入 {} → {}", input, scheme.as_str());
    for item in outputs {
        let tier = match item.tier {
            PriorityTier::Primary => "primary",
            PriorityTier::Secondary => "secondary",
            PriorityTier::Fallback => "fallback",
        };
        let tone = item
            .tone
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let applied = if item.applied.is_empty() {
            "identity".to_string()
        } else {
            item.applied.join(",")
        };
        println!("{}\t调{}\t{}\t{}", item.text, tone, tier, applied);
    }
}

fn tone(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("classes") => {
            let (positional, flags) = split_positional(&rest[1..]);
            if !positional.is_empty() {
                return Err("tone classes 不接受额外位置参数".into());
            }
            let resource = resolve_region(manifest_path, &flags)?;
            if resource.config.tone_classes.is_empty() {
                println!("区域 {} 未声明 tone_classes", resource.entry.id);
                return Ok(());
            }
            for (id, class) in &resource.config.tone_classes {
                println!("{}\t{}\t{}", id, class.name, class.value);
            }
            Ok(())
        }
        Some(unknown) => Err(format!("未知 tone 子命令: {unknown}").into()),
        None => Err("tone 缺少子命令；可用：classes".into()),
    }
}

fn checked(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("alternatives") => {
            let (positional, flags) = split_positional(&rest[1..]);
            let syllable = positional
                .first()
                .ok_or("checked alternatives 缺少 syllable")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let book = load_pronunciation_book(&resource)?;
            let alternatives = book.checked_alternatives(syllable);
            if alternatives.is_empty() {
                println!("未发现 {} 的入声候选", syllable);
                return Ok(());
            }
            for syllable_text in alternatives {
                println!("{}", syllable_text);
            }
            Ok(())
        }
        Some(unknown) => Err(format!("未知 checked 子命令: {unknown}").into()),
        None => Err("checked 缺少子命令；可用：alternatives".into()),
    }
}

fn mandarin(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("hint") => {
            let (positional, flags) = split_positional(&rest[1..]);
            let term = positional.first().ok_or("mandarin hint 缺少 term")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let book = load_mandarin_hints(&resource)?;
            let entries = book.lookup_by_mandarin(term);
            if entries.is_empty() {
                println!("未命中: {}", term);
                return Ok(());
            }
            for entry in entries {
                let reading = entry.reading.clone().unwrap_or_default();
                let note = entry.note.clone().unwrap_or_default();
                println!("{}\t{}\t{}\t{}", entry.mandarin, entry.gan, reading, note);
            }
            Ok(())
        }
        Some(unknown) => Err(format!("未知 mandarin 子命令: {unknown}").into()),
        None => Err("mandarin 缺少子命令；可用：hint".into()),
    }
}

fn register(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("check") => {
            let (positional, flags) = split_positional(&rest[1..]);
            let grapheme = positional.first().ok_or("register check 缺少 grapheme")?;
            let syllable_text = positional.get(1).ok_or("register check 缺少 syllable")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let book = load_pronunciation_book(&resource)?;
            match book.register_correction(grapheme, syllable_text) {
                None => {
                    println!("{} 读 {} 未发现 register 纠正", grapheme, syllable_text);
                }
                Some(correction) => {
                    println!(
                        "{} 读 {} 当前 register={}",
                        correction.grapheme,
                        correction.observed_syllable,
                        register_label(correction.observed_register)
                    );
                    for alternate in &correction.alternates {
                        let tone = alternate
                            .tone_class
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "  → {}\t{}\t调{}",
                            alternate.syllable,
                            register_label(alternate.register),
                            tone
                        );
                    }
                }
            }
            Ok(())
        }
        Some(unknown) => Err(format!("未知 register 子命令: {unknown}").into()),
        None => Err("register 缺少子命令；可用：check".into()),
    }
}

fn register_label(register: Register) -> &'static str {
    match register {
        Register::Wen => "wen",
        Register::Bai => "bai",
        Register::Common => "common",
    }
}

fn split_positional(args: &[String]) -> (Vec<&String>, Vec<&String>) {
    let mut positional = Vec::new();
    let mut flags = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        if token.starts_with("--") {
            flags.push(token);
            if !token.contains('=') {
                if let Some(next) = args.get(index + 1) {
                    flags.push(next);
                    index += 2;
                    continue;
                }
            }
            index += 1;
        } else {
            positional.push(token);
            index += 1;
        }
    }
    (positional, flags)
}

fn resolve_region(
    manifest_path: &Path,
    flags: &[&String],
) -> Result<RegionResource, Box<dyn Error>> {
    let mut region_id: Option<String> = None;
    let mut index = 0;
    while index < flags.len() {
        let token = flags[index].as_str();
        if token == "--region" {
            let value = flags.get(index + 1).ok_or("--region 缺少参数值")?;
            region_id = Some(value.to_string());
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--region=") {
            region_id = Some(value.to_string());
            index += 1;
            continue;
        }
        return Err(format!("未知参数: {token}").into());
    }
    let target = match region_id {
        Some(value) => value,
        None => default_region_entry(manifest_path)?.id,
    };
    Ok(load_region_from_manifest(manifest_path, &target)?)
}

fn load_region_dictionary(resource: &RegionResource) -> Result<Dictionary, Box<dyn Error>> {
    let paths: Vec<PathBuf> = [
        resource.config.language.chars.as_deref(),
        resource.config.language.words.as_deref(),
        resource.config.language.gan_chars.as_deref(),
        resource.config.language.gan_words.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(|relative| resource.root.join(relative))
    .collect();
    let index_dir = resource
        .root
        .parent()
        .and_then(Path::parent)
        .map(|path| path.join("indexes"));
    if !paths.is_empty() {
        return Ok(Dictionary::load_split_tsvs(&paths, index_dir.as_deref())?);
    }
    Ok(Dictionary::empty())
}

fn load_pronunciation_book(resource: &RegionResource) -> Result<PronunciationBook, Box<dyn Error>> {
    let mut book = PronunciationBook::empty();
    let dictionary = load_region_dictionary(resource)?;
    if !dictionary.is_empty() {
        book.extend_dictionary(&dictionary);
    }
    if let Some(relative) = resource.config.phonology.pronunciations.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.extend_from_jsonl(&path)?;
        }
    }
    Ok(book)
}

fn load_mandarin_hints(resource: &RegionResource) -> Result<MandarinHintBook, Box<dyn Error>> {
    let mut book = MandarinHintBook::empty();
    let dictionary = load_region_dictionary(resource)?;
    if !dictionary.is_empty() {
        book.extend_dictionary(&dictionary);
    }
    if let Some(relative) = resource.config.dictionaries.feature_words.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.extend(MandarinHintBook::load_feature_words_tsv(&path)?);
        }
    }
    if let Some(relative) = resource.config.dictionaries.mandarin_hints.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.extend(MandarinHintBook::load_jsonl(&path)?);
        }
    }
    Ok(book)
}

fn file_has_content(path: &Path) -> Result<bool, std::io::Error> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len() > 0)
}

fn lookup(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    let (positional, flags) = split_positional(rest);
    let text = positional.first().ok_or("lookup 缺少 text")?;
    let resource = resolve_region(manifest_path, &flags)?;
    let book = load_slang_book(&resource)?;

    let slang_hits = book.slang_by_trigger(text);
    let assoc_hits = book.association_by_trigger(text);
    let reverse_hits = book.slang_reverse(text);

    if slang_hits.is_empty() && assoc_hits.is_empty() && reverse_hits.is_empty() {
        println!("未命中: {}", text);
        return Ok(());
    }

    if !slang_hits.is_empty() {
        println!("# 俚语正向");
        for hit in &slang_hits {
            let reading = hit.entry.slang_reading.clone().unwrap_or_default();
            let kind = trigger_kind_label(hit.matched_trigger.kind);
            println!(
                "{}\t{}\t{}\t触发={} ({})",
                hit.entry.slang,
                reading,
                hit.entry.mandarin_glosses.join("/"),
                hit.matched_trigger.text,
                kind
            );
        }
    }

    if !assoc_hits.is_empty() {
        println!("# 联想");
        for hit in &assoc_hits {
            for suggestion in &hit.entry.suggestions {
                let reading = suggestion.reading.clone().unwrap_or_default();
                let relation = suggestion.relation.clone().unwrap_or_default();
                let fragment = if suggestion.is_fragment {
                    "fragment"
                } else {
                    "full"
                };
                println!(
                    "{} → {}\t{}\t{}\t{}",
                    hit.entry.trigger, suggestion.text, reading, relation, fragment
                );
            }
        }
    }

    if !reverse_hits.is_empty() {
        println!("# 俚语反查（过滤截片）");
        for hit in &reverse_hits {
            for trigger in &hit.triggers {
                let kind = trigger_kind_label(trigger.kind);
                println!("{} ← {}\t{}", hit.entry.slang, trigger.text, kind);
            }
        }
    }

    Ok(())
}

fn trigger_kind_label(kind: TriggerKind) -> &'static str {
    match kind {
        TriggerKind::Mandarin => "mandarin",
        TriggerKind::GanVocab => "gan-vocab",
        TriggerKind::GanFragment => "gan-fragment",
    }
}

fn load_slang_book(resource: &RegionResource) -> Result<SlangBook, Box<dyn Error>> {
    let mut book = SlangBook::empty();
    let dictionary = load_region_dictionary(resource)?;
    if !dictionary.is_empty() {
        book.load_dictionary(&dictionary);
    }
    if let Some(relative) = resource.config.dictionaries.feature_words.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.load_feature_words_tsv(&path)?;
        }
    }
    if let Some(relative) = resource.config.dictionaries.slang.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.load_slang_jsonl(&path)?;
        }
    }
    if let Some(relative) = resource.config.dictionaries.associations.as_deref() {
        let path = resource.root.join(relative);
        if path.is_file() && file_has_content(&path)? {
            book.load_association_jsonl(&path)?;
        }
    }
    Ok(book)
}

fn pipeline(manifest_path: &Path, rest: &[String]) -> Result<(), Box<dyn Error>> {
    match rest.first().map(String::as_str) {
        Some("compose") => {
            let (positional, flags) = split_positional(&rest[1..]);
            let input = positional.first().ok_or("pipeline compose 缺少 input")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let pipeline = InputPipeline::load(&resource)?;
            let composed = pipeline.compose(input);
            if composed.is_empty() {
                println!("未命中: {}", input);
                return Ok(());
            }
            for item in composed {
                print_composed(&item);
            }
            Ok(())
        }
        Some("retrieve") => {
            let (positional, flags) = split_positional(&rest[1..]);
            let input = positional.first().ok_or("pipeline retrieve 缺少 input")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let pipeline = InputPipeline::load(&resource)?;
            let ranked = pipeline.retrieve(input);
            if ranked.is_empty() {
                println!("未命中: {}", input);
                return Ok(());
            }
            for item in ranked {
                print_ranked(&item);
            }
            Ok(())
        }
        Some("batch") => {
            // 批量回归比对: 每行一个输入, 依次输出 retrieve+compose, 词典只加载一次。
            let (positional, flags) = split_positional(&rest[1..]);
            let list_path = positional
                .first()
                .ok_or("pipeline batch 缺少输入清单文件")?;
            let resource = resolve_region(manifest_path, &flags)?;
            let pipeline = InputPipeline::load(&resource)?;
            let list = std::fs::read_to_string(list_path)?;
            for input in list.lines().map(str::trim).filter(|l| !l.is_empty()) {
                println!("##### [{input}] retrieve");
                let ranked = pipeline.retrieve(input);
                if ranked.is_empty() {
                    println!("未命中: {input}");
                } else {
                    for item in ranked {
                        print_ranked(&item);
                    }
                }
                println!("##### [{input}] compose");
                let composed = pipeline.compose(input);
                if composed.is_empty() {
                    println!("未命中: {input}");
                } else {
                    for item in composed {
                        print_composed(&item);
                    }
                }
            }
            Ok(())
        }
        Some(unknown) => Err(format!("未知 pipeline 子命令: {unknown}").into()),
        None => Err("pipeline 缺少子命令；可用：compose、retrieve、batch".into()),
    }
}

fn print_composed(item: &ComposedCandidate) {
    let source = match item.source {
        CandidateSource::Slang => "slang",
        CandidateSource::Association => "association",
        CandidateSource::SlangReverse => "slang-reverse",
        CandidateSource::MandarinHint => "mandarin-hint",
        CandidateSource::Pronunciation => "pronunciation",
    };
    let tier = match item.tier {
        CandidateTier::Primary => "primary",
        CandidateTier::Secondary => "secondary",
        CandidateTier::Fallback => "fallback",
    };
    let reading = item.reading.clone().unwrap_or_default();
    let note = item.note.clone().unwrap_or_default();
    println!(
        "{}\t{}\t{}\t{}\t{:.3}\t{}",
        item.text, reading, source, tier, item.weight, note
    );
}

fn print_ranked(item: &RankedCandidate) {
    let layer = match item.layer {
        RetrievalLayer::GannyuExact => "gan-exact",
        RetrievalLayer::MandarinExact => "mandarin-exact",
        RetrievalLayer::Fuzzy => "fuzzy",
        RetrievalLayer::Synonym => "synonym",
    };
    let display = if let Some(annotation) = &item.annotation {
        format!("{} ({})", item.text, annotation)
    } else if let Some(reading) = &item.reading {
        format!("{} ({})", item.text, reading)
    } else if item.mandarin_only {
        format!("{}[官]", item.text)
    } else {
        item.text.clone()
    };
    let ipa = item.ipa.clone().unwrap_or_default();
    println!("{}\t{}\t{:.3}\t{}", display, layer, item.weight, ipa);
}
