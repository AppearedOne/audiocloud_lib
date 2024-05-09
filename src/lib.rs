use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
use std::usize;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    pub samples: Vec<Sample>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchParams {
    pub query: String,
    pub sample_type: Option<SampleType>,
    pub max_tempo: Option<u32>,
    pub min_tempo: Option<u32>,
    pub pack_id: Option<String>,
    pub max_results: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub enum SampleType {
    Loop(i32),
    OneShot,
}

#[derive(Debug, Serialize, Deserialize, Eq, Ord, PartialEq, PartialOrd, Clone)]
pub struct Sample {
    pub path: String,
    pub name: String,
    pub sampletype: SampleType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackInfo {
    pub description: String,
    pub name: String,
    pub img: Option<String>,
    num_samples: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pack {
    pub samples: Vec<Sample>,
    pub meta: PackInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SampleLibrary {
    pub packs: Vec<Pack>,
    pub name: String,
}

pub fn get_packs_metadata(lib: &SampleLibrary) -> Vec<PackInfo> {
    let mut out: Vec<PackInfo> = vec![];
    for pack in &lib.packs {
        out.push(pack.meta.clone());
    }
    out
}

pub fn use_sample_relevance(
    query: &SearchParams,
    sample: &Sample,
    text_queries: &Vec<&str>,
) -> i32 {
    // print!("New Sample: {}", sample.name);
    if query.sample_type.is_some() {
        if std::mem::discriminant(query.sample_type.as_ref().unwrap())
            != std::mem::discriminant(&sample.sampletype)
        {
            // println!("Ignoring because of sampletype");
            return 0;
        }
    }

    if query.sample_type.is_some() {
        match query.sample_type.as_ref().unwrap() {
            SampleType::Loop(tempo) => {
                if query.min_tempo.is_some() && tempo.to_owned() as u32 > query.min_tempo.unwrap() {
                    println!("TEMPO TO LOW");
                    return 0;
                }
                if query.max_tempo.is_some() && (tempo.to_owned() as u32) < query.max_tempo.unwrap()
                {
                    println!("TEMPO TO HIGH");
                    return 0;
                }
            }
            SampleType::OneShot => (),
        }
    }

    let mut relevancy = 0;
    let mut is_filtered = false;
    text_queries.iter().for_each(|token| {
        if !token.is_empty() {
            if token.starts_with('-') {
                if sample
                    .path
                    .to_lowercase()
                    .contains(&token.to_lowercase().replace("-", ""))
                {
                    is_filtered = true;
                }
            }
            if sample.path.to_lowercase().contains(&token.to_lowercase()) {
                relevancy += 1;
            }
        }
    });

    if is_filtered {
        return 0;
    }
    return relevancy;
}

pub fn search_lib(lib: &SampleLibrary, query: &SearchParams) -> SearchResult {
    let query_lowercase = query.query.to_lowercase();
    let mut text_queries: Vec<&str> = query_lowercase.split(' ').collect();
    text_queries.iter_mut().for_each(|s| *s = s.trim());
    /* text_queries
    .iter()
    .for_each(|q| println!("QUERYUNIQUE: {}", q));*/

    let mut sorting_vec: Vec<(Sample, i32)> = vec![];
    for pack in &lib.packs {
        if query.pack_id.is_some() {
            if !pack.meta.name.eq(query.pack_id.as_ref().unwrap()) {
                continue;
            }
        }

        pack.samples.iter().for_each(|sample| {
            let rev = use_sample_relevance(&query, &sample, &text_queries);
            if rev > 0 {
                sorting_vec.push(((*sample).clone(), rev));
            }
        });
    }

    sorting_vec.sort_by(|a, b| b.1.cmp(&a.1));

    let max_results: usize = match query.max_results {
        Some(input) => input as usize,
        None => 10,
    };

    let ret_vec: Vec<Sample>;
    if sorting_vec.len() >= max_results {
        ret_vec = sorting_vec
            .into_iter()
            .take(max_results)
            .map(|(e, _)| e)
            .collect();
    } else {
        ret_vec = sorting_vec.into_iter().map(|element| element.0).collect();
    }

    SearchResult { samples: ret_vec }
}

fn extract_tempo_braces(path: &str) -> Option<i32> {
    let input = path;
    let start_index = input.find('[')?;
    let end_index = input[start_index + 1..].find(']')? + start_index + 1;

    input[start_index + 1..end_index].parse().ok()
}

fn detect_tempo_txt(path: &str) -> i32 {
    let extracted = extract_tempo_braces(path);
    match extracted {
        Some(val) => {
            return val;
        }
        None => {
            return 0;
        }
    }
}

fn detect_type(path: &str) -> SampleType {
    let loop_signals = [
        "/loop",
        "/construction",
        "_loop",
        "[",
        "bpm",
        "loop",
        "loops",
    ];
    // todo: load keywords from json
    let path_lower = path.to_lowercase();
    for keyword in loop_signals {
        if path_lower.contains(keyword) {
            let tempo = detect_tempo_txt(&path_lower);
            return SampleType::Loop(tempo);
        }
    }
    SampleType::OneShot
}

pub fn get_sample(path: &str) -> Sample {
    let sample_type = detect_type(path);
    let sample = Sample {
        name: path.to_string(), // TODO: CUT OFF EVERYTHING BEFORE THE LAST "/"
        path: path.to_string(),
        sampletype: sample_type,
    };
    sample
}

pub fn load_pack(path: &str, name: &str, desc: &str) -> Pack {
    let start_path = path.to_string();
    let mut count_loop = 0;
    let mut count_oneshot = 0;
    let mut pack = Pack {
        samples: vec![],
        meta: PackInfo {
            description: desc.to_string(),
            name: name.to_string(),
            img: None,
            num_samples: None,
        },
    };

    for entry in WalkDir::new(start_path).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path().display().to_string();
        let entry_name = entry
            .file_name()
            .to_str()
            .expect("Couldnt get entry name")
            .to_string();

        if entry_name.contains(".wav") || entry_name.contains(".mp3") {
            let stype = detect_type(&entry_path.to_lowercase());
            match stype {
                SampleType::OneShot => {
                    count_oneshot += 1;
                }
                SampleType::Loop(_) => {
                    count_loop += 1;
                }
            }
            pack.samples.push(Sample {
                path: entry_path,
                name: entry_name.clone(),
                sampletype: stype,
            });
            println!("Sample found: {}", &entry_name);
        }
    }
    println!("Loops: {count_loop}, OneShots: {count_oneshot}");
    pack.meta.num_samples = Some(
        pack.samples
            .len()
            .try_into()
            .expect("Overflow: Too many samples in pack for u32"),
    );
    pack
}

pub fn save_lib_json(lib: &SampleLibrary, folder_path: &str) {
    let json_lib = serde_json::to_string_pretty(&lib).expect("Couldnt create json data!");
    let mut file =
        File::create(folder_path.to_string() + &lib.name + ".json").expect("Couldnt open file!!");
    file.write(json_lib.as_bytes())
        .expect("Couldnt write file!!");
}

pub fn load_lib_json(path: &str) -> SampleLibrary {
    let content = std::fs::read_to_string(path).expect("Couldn't read json file");
    let lib: SampleLibrary = serde_json::from_str(&content).expect("Couldn't parse json");
    lib
}
