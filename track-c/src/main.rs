// Track C — Speed Demon Hackathon (Join)
// Parse records.csv (hostile CSV) + join with regions.csv -> aggregates.txt
//
// Zero external dependencies. Fast, byte-exact, deterministic.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{BuildHasher, Hasher};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub struct FnvHasher(u64);

impl Hasher for FnvHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x00000100000001B3);
        }
        self.0 = h;
    }
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }
}

#[derive(Default, Clone, Copy)]
pub struct FnvBuildHasher;

impl BuildHasher for FnvBuildHasher {
    type Hasher = FnvHasher;
    #[inline(always)]
    fn build_hasher(&self) -> FnvHasher {
        FnvHasher(0xcbf29ce484222325)
    }
}

type FnvHashSet<T> = HashSet<T, FnvBuildHasher>;

const BOM: &[u8] = b"\xef\xbb\xbf";
const UNMATCHED_NAME: &str = "_UNMATCHED";

#[derive(Default)]
struct GroupAgg {
    count: u64,
    id_sum: u64,
    amount_sum: i64,
    amount_min: i64,
    amount_max: i64,
    name_bytes: u64,
    note_bytes: u64,
    names: FnvHashSet<Box<[u8]>>,
}

impl GroupAgg {
    fn new() -> Self {
        GroupAgg {
            count: 0,
            id_sum: 0,
            amount_sum: 0,
            amount_min: i64::MAX,
            amount_max: i64::MIN,
            name_bytes: 0,
            note_bytes: 0,
            names: HashSet::with_hasher(FnvBuildHasher),
        }
    }

    #[inline]
    fn add(&mut self, record_id: u64, name: &[u8], note_len: usize, cents: i64) {
        self.count += 1;
        self.id_sum += record_id;
        self.amount_sum += cents;
        if cents < self.amount_min {
            self.amount_min = cents;
        }
        if cents > self.amount_max {
            self.amount_max = cents;
        }
        self.name_bytes += name.len() as u64;
        self.note_bytes += note_len as u64;

        if !self.names.contains(name) {
            self.names.insert(name.into());
        }
    }
}

/// Format cents as decimal with 2 digits: e.g. -525 -> "-5.25", 0 -> "0.00"
#[inline]
fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{}{}.{:02}", sign, abs / 100, abs % 100)
}

struct RegionsLookup {
    country_names: Vec<String>,
    unmatched_idx: usize,
    region_to_country: Vec<usize>,
    large_regions: HashMap<usize, usize>,
}

fn parse_regions(data: &[u8]) -> RegionsLookup {
    let mut i = 0;
    let len = data.len();

    // Skip possible leading BOM
    if data[i..].starts_with(BOM) {
        i += 3;
    }

    // Skip header line
    while i < len && data[i] != b'\n' {
        i += 1;
    }
    if i < len && data[i] == b'\n' {
        i += 1;
    }

    let mut raw_pairs = Vec::with_capacity(200_000);
    let mut country_set = HashSet::new();
    let mut max_region_id = 0usize;

    while i < len {
        // Skip empty lines or trailing terminators
        if data[i] == b'\r' || data[i] == b'\n' {
            i += 1;
            continue;
        }

        // Parse region_id
        let mut rid = 0usize;
        while i < len && data[i] >= b'0' && data[i] <= b'9' {
            rid = rid * 10 + (data[i] - b'0') as usize;
            i += 1;
        }
        if i < len && data[i] == b',' {
            i += 1; // skip comma
        }

        // Parse country (3 ASCII letters)
        let c_start = i;
        while i < len && data[i] != b'\r' && data[i] != b'\n' {
            i += 1;
        }
        let country_str = String::from_utf8_lossy(&data[c_start..i]).into_owned();

        // Skip line terminator
        if i < len && data[i] == b'\r' {
            i += 1;
        }
        if i < len && data[i] == b'\n' {
            i += 1;
        }

        if !country_str.is_empty() {
            if rid > max_region_id && rid <= 2_000_000 {
                max_region_id = rid;
            }
            country_set.insert(country_str.clone());
            raw_pairs.push((rid, country_str));
        }
    }

    let mut country_names: Vec<String> = country_set.into_iter().collect();
    country_names.sort(); // bytewise ascending

    let mut country_to_idx = HashMap::new();
    for (idx, name) in country_names.iter().enumerate() {
        country_to_idx.insert(name.clone(), idx);
    }

    let unmatched_idx = country_names.len();
    country_names.push(UNMATCHED_NAME.to_string());

    let mut region_to_country = vec![unmatched_idx; max_region_id + 1];
    let mut large_regions = HashMap::new();

    for (rid, cname) in raw_pairs {
        let c_idx = country_to_idx[&cname];
        if rid <= max_region_id {
            region_to_country[rid] = c_idx;
        } else {
            large_regions.insert(rid, c_idx);
        }
    }

    RegionsLookup {
        country_names,
        unmatched_idx,
        region_to_country,
        large_regions,
    }
}

fn process_records(records_data: &[u8], lookup: &RegionsLookup, groups: &mut [GroupAgg]) {
    let mut i = 0;
    let len = records_data.len();

    // Skip optional leading BOM
    if records_data[i..].starts_with(BOM) {
        i += 3;
    }

    // Skip header line
    while i < len && records_data[i] != b'\n' {
        i += 1;
    }
    if i < len && records_data[i] == b'\n' {
        i += 1;
    }

    let mut scratch_name: Vec<u8> = Vec::with_capacity(256);

    while i < len {
        // Skip possible mid-file BOM at start of record
        if i + 3 <= len && &records_data[i..i + 3] == BOM {
            i += 3;
        }

        // Check for end of file or stray empty lines
        if i >= len || records_data[i] == b'\r' || records_data[i] == b'\n' {
            if i < len {
                i += 1;
            }
            continue;
        }

        // 1. Parse id
        let mut record_id: u64 = 0;
        while i < len && records_data[i] >= b'0' && records_data[i] <= b'9' {
            record_id = record_id * 10 + (records_data[i] - b'0') as u64;
            i += 1;
        }
        if i < len && records_data[i] == b',' {
            i += 1; // skip ','
        }

        // 2. Parse name
        let name_bytes: &[u8] = if i < len && records_data[i] != b'"' {
            // Unquoted: empty
            while i < len && records_data[i] != b',' {
                i += 1;
            }
            if i < len && records_data[i] == b',' {
                i += 1; // skip ','
            }
            &b""[..]
        } else {
            // Quoted
            i += 1; // skip opening '"'
            let content_start = i;
            let mut has_escapes = false;
            while i < len {
                if records_data[i] == b'"' {
                    if i + 1 < len && records_data[i + 1] == b'"' {
                        has_escapes = true;
                        i += 2;
                    } else {
                        // Closing quote
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            let content_end = i;
            if i < len && records_data[i] == b'"' {
                i += 1; // skip closing '"'
            }
            if i < len && records_data[i] == b',' {
                i += 1; // skip ','
            }

            if !has_escapes {
                &records_data[content_start..content_end]
            } else {
                scratch_name.clear();
                let mut j = content_start;
                while j < content_end {
                    if records_data[j] == b'"' && j + 1 < content_end && records_data[j + 1] == b'"'
                    {
                        scratch_name.push(b'"');
                        j += 2;
                    } else {
                        scratch_name.push(records_data[j]);
                        j += 1;
                    }
                }
                &scratch_name[..]
            }
        };

        // 3. Parse note
        let note_len: usize = if i < len && records_data[i] != b'"' {
            // Unquoted: empty
            while i < len && records_data[i] != b',' {
                i += 1;
            }
            if i < len && records_data[i] == b',' {
                i += 1; // skip ','
            }
            0
        } else {
            // Quoted
            i += 1; // skip opening '"'
            let content_start = i;
            let mut escaped_quotes = 0;
            while i < len {
                if records_data[i] == b'"' {
                    if i + 1 < len && records_data[i + 1] == b'"' {
                        escaped_quotes += 1;
                        i += 2;
                    } else {
                        // Closing quote
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            let content_end = i;
            if i < len && records_data[i] == b'"' {
                i += 1; // skip closing '"'
            }
            if i < len && records_data[i] == b',' {
                i += 1; // skip ','
            }
            (content_end - content_start) - escaped_quotes
        };

        // 4. Parse region_id
        let mut region_id: usize = 0;
        while i < len && records_data[i] >= b'0' && records_data[i] <= b'9' {
            region_id = region_id * 10 + (records_data[i] - b'0') as usize;
            i += 1;
        }
        if i < len && records_data[i] == b',' {
            i += 1; // skip ','
        }

        // 5. Parse amount
        let is_negative = if i < len && records_data[i] == b'-' {
            i += 1;
            true
        } else {
            false
        };
        let mut whole: i64 = 0;
        while i < len && records_data[i] >= b'0' && records_data[i] <= b'9' {
            whole = whole * 10 + (records_data[i] - b'0') as i64;
            i += 1;
        }
        let cents: i64 = if i < len && records_data[i] == b'.' {
            i += 1; // skip '.'
            let d1 = if i < len {
                (records_data[i] - b'0') as i64
            } else {
                0
            };
            let d2 = if i + 1 < len {
                (records_data[i + 1] - b'0') as i64
            } else {
                0
            };
            i += 2;
            whole * 100 + d1 * 10 + d2
        } else {
            whole * 100
        };
        let cents = if is_negative { -cents } else { cents };

        // Skip line terminator
        if i < len && records_data[i] == b'\r' {
            i += 1;
        }
        if i < len && records_data[i] == b'\n' {
            i += 1;
        }

        // Join to country
        let country_idx = if region_id < lookup.region_to_country.len() {
            lookup.region_to_country[region_id]
        } else {
            lookup
                .large_regions
                .get(&region_id)
                .copied()
                .unwrap_or(lookup.unmatched_idx)
        };

        groups[country_idx].add(record_id, name_bytes, note_len, cents);
    }
}

fn locate_file(paths: &[&str]) -> Option<PathBuf> {
    for p in paths {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    None
}

fn run_pipeline(records_bytes: &[u8], regions_bytes: &[u8]) -> Vec<u8> {
    let lookup = parse_regions(regions_bytes);
    let mut groups: Vec<GroupAgg> = (0..lookup.country_names.len())
        .map(|_| GroupAgg::new())
        .collect();

    process_records(records_bytes, &lookup, &mut groups);

    let mut out_bytes = Vec::with_capacity(16 * 1024);
    for (idx, country_name) in lookup.country_names.iter().enumerate() {
        let g = &groups[idx];
        if g.count > 0 {
            let line = format!(
                "{} {} {} {} {} {} {} {} {}\n",
                country_name,
                g.count,
                g.id_sum,
                format_cents(g.amount_sum),
                format_cents(g.amount_min),
                format_cents(g.amount_max),
                g.names.len(),
                g.name_bytes,
                g.note_bytes,
            );
            out_bytes.extend_from_slice(line.as_bytes());
        }
    }
    out_bytes
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let records_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        locate_file(&["/data/records.csv", "/data/c/records.csv"])
            .unwrap_or_else(|| PathBuf::from("/data/records.csv"))
    };
    let regions_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        locate_file(&["/data/regions.csv", "/data/c/regions.csv"])
            .unwrap_or_else(|| PathBuf::from("/data/regions.csv"))
    };
    let out_path = if args.len() > 3 {
        let p = PathBuf::from(&args[3]);
        if p.is_dir() {
            p.join("aggregates.txt")
        } else {
            p
        }
    } else {
        PathBuf::from("/out/aggregates.txt")
    };

    let records_data = fs::read(&records_path).expect("failed to read records.csv");
    let regions_data = fs::read(&regions_path).expect("failed to read regions.csv");

    let out_bytes = run_pipeline(&records_data, &regions_data);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let out_file = File::create(&out_path).expect("failed to create output file");
    let mut writer = BufWriter::new(out_file);
    writer
        .write_all(&out_bytes)
        .expect("failed to write output");
    writer.flush().expect("failed to flush output");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(slices: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for s in slices {
            v.extend_from_slice(s);
        }
        v
    }

    fn get_worked_records() -> Vec<u8> {
        cat(&[
            b"\xef\xbb\xbfid,name,note,region_id,amount\r\n",
            b"1,\"Kaphar, Titus\",\"said \"\"no\"\" twice\",7,1420.00\r\n",
            b"2,\"Betts\",\"a note that\nspans two lines\",7,88.50\n",
            b"3,,\"\",12,0.00\r\n",
            b"4,\"trailing space \",\"comma, quote \"\" and newline\ntogether\",3,-5.25\n",
            b"\xef\xbb\xbf5,\"Betts \",\"one\ntwo\nthree\",12,-0.75\r\n",
            b"6,\"\",,12,10.00\r\n",
            b"\xef\xbb\xbf7,\"Ann \"\"Annie\"\" Lee\",\"x\",5,2.00\n",
            b"8,\"Nobody\",\"\",4000000,-1.00\r\n",
            b"9,\"Neg\",\"n\",20,-0.05\r\n",
        ])
    }

    const WORKED_REGIONS: &[u8] = b"region_id,country\n3,BBB\n7,AAA\n12,AAA\n20,CCC\n21,DDD\n";

    fn get_worked_output() -> Vec<u8> {
        cat(&[
            b"AAA 5 17 1517.75 -0.75 1420.00 4 24 55\n",
            b"BBB 1 4 -5.25 -5.25 -5.25 1 15 35\n",
            b"CCC 1 9 -0.05 -0.05 -0.05 1 3 1\n",
            b"_UNMATCHED 2 15 1.00 -1.00 2.00 2 21 1\n",
        ])
    }

    fn get_rules_records() -> Vec<u8> {
        cat(&[
            b"id,name,note,region_id,amount\n",
            b"1,\"a\xef\xbb\xbfb\",\"x\n\xef\xbb\xbfy\",7,1.00\n",
            b"2,\"\xef\xbb\xbf\",\"\",7,-1.00\r\n",
            b"4,\"d\",\"\",8,5.00\n",
            b"5,\"d \",\"\",8,-5.00\r\n",
            b"\xef\xbb\xbf3,\"c\",,7,0.50",
        ])
    }
    const RULES_REGIONS: &[u8] = b"region_id,country\n7,AAA\n8,BBB\n";

    fn get_rules_output() -> Vec<u8> {
        cat(&[
            b"AAA 3 6 0.50 -1.00 1.00 3 9 6\n",
            b"BBB 2 9 0.00 -5.00 5.00 2 3 0\n",
        ])
    }

    #[test]
    fn test_format_cents() {
        assert_eq!(format_cents(0), "0.00");
        assert_eq!(format_cents(5), "0.05");
        assert_eq!(format_cents(-5), "-0.05");
        assert_eq!(format_cents(99), "0.99");
        assert_eq!(format_cents(-100), "-1.00");
        assert_eq!(format_cents(-525), "-5.25");
        assert_eq!(format_cents(142000), "1420.00");
        assert_eq!(format_cents(123456789), "1234567.89");
    }

    #[test]
    fn test_worked_example() {
        let recs = get_worked_records();
        let expected = get_worked_output();
        let got = run_pipeline(&recs, WORKED_REGIONS);
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            std::str::from_utf8(&expected).unwrap()
        );
    }

    #[test]
    fn test_rules_records() {
        let recs = get_rules_records();
        let expected = get_rules_output();
        let got = run_pipeline(&recs, RULES_REGIONS);
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            std::str::from_utf8(&expected).unwrap()
        );
    }

    #[test]
    fn test_unmatched_sorts_last() {
        let recs = cat(&[
            b"\xef\xbb\xbfid,name,note,region_id,amount\r\n",
            b"1,\"x\",\"\",1,1.00\n",
            b"2,\"x\",\"\",2,1.00\n",
            b"3,\"x\",\"\",3,1.00\n",
            b"4,\"x\",\"\",9,1.00\r\n",
        ]);
        let regs = b"region_id,country\n1,ZZZ\n2,AAB\n3,AAA\n";
        let expected = cat(&[
            b"AAA 1 3 1.00 1.00 1.00 1 1 0\n",
            b"AAB 1 2 1.00 1.00 1.00 1 1 0\n",
            b"ZZZ 1 1 1.00 1.00 1.00 1 1 0\n",
            b"_UNMATCHED 1 4 1.00 1.00 1.00 1 1 0\n",
        ]);
        let got = run_pipeline(&recs, regs);
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            std::str::from_utf8(&expected).unwrap()
        );
    }
}
