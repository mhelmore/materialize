// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE file at the
// root of this repository, or online at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// BEGIN LINT CONFIG
// DO NOT EDIT. Automatically generated by bin/gen-lints.
// Have complaints about the noise? See the note in misc/python/materialize/cli/gen-lints.py first.
#![allow(unknown_lints)]
#![allow(clippy::style)]
#![allow(clippy::complexity)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::mutable_key_type)]
#![allow(clippy::stable_sort_primitive)]
#![allow(clippy::map_entry)]
#![allow(clippy::box_default)]
#![allow(clippy::drain_collect)]
#![warn(clippy::bool_comparison)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::no_effect)]
#![warn(clippy::unnecessary_unwrap)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::todo)]
#![warn(clippy::wildcard_dependencies)]
#![warn(clippy::zero_prefixed_literal)]
#![warn(clippy::borrowed_box)]
#![warn(clippy::deref_addrof)]
#![warn(clippy::double_must_use)]
#![warn(clippy::double_parens)]
#![warn(clippy::extra_unused_lifetimes)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::needless_question_mark)]
#![warn(clippy::needless_return)]
#![warn(clippy::redundant_pattern)]
#![warn(clippy::redundant_slicing)]
#![warn(clippy::redundant_static_lifetimes)]
#![warn(clippy::single_component_path_imports)]
#![warn(clippy::unnecessary_cast)]
#![warn(clippy::useless_asref)]
#![warn(clippy::useless_conversion)]
#![warn(clippy::builtin_type_shadow)]
#![warn(clippy::duplicate_underscore_argument)]
#![warn(clippy::double_neg)]
#![warn(clippy::unnecessary_mut_passed)]
#![warn(clippy::wildcard_in_or_patterns)]
#![warn(clippy::crosspointer_transmute)]
#![warn(clippy::excessive_precision)]
#![warn(clippy::overflow_check_conditional)]
#![warn(clippy::as_conversions)]
#![warn(clippy::match_overlapping_arm)]
#![warn(clippy::zero_divided_by_zero)]
#![warn(clippy::must_use_unit)]
#![warn(clippy::suspicious_assignment_formatting)]
#![warn(clippy::suspicious_else_formatting)]
#![warn(clippy::suspicious_unary_op_formatting)]
#![warn(clippy::mut_mutex_lock)]
#![warn(clippy::print_literal)]
#![warn(clippy::same_item_push)]
#![warn(clippy::useless_format)]
#![warn(clippy::write_literal)]
#![warn(clippy::redundant_closure)]
#![warn(clippy::redundant_closure_call)]
#![warn(clippy::unnecessary_lazy_evaluations)]
#![warn(clippy::partialeq_ne_impl)]
#![warn(clippy::redundant_field_names)]
#![warn(clippy::transmutes_expressible_as_ptr_casts)]
#![warn(clippy::unused_async)]
#![warn(clippy::disallowed_methods)]
#![warn(clippy::disallowed_macros)]
#![warn(clippy::disallowed_types)]
#![warn(clippy::from_over_into)]
// END LINT CONFIG

use std::path::PathBuf;
use std::{env, fs};

use anyhow::{bail, Context, Result};
use chrono_tz::TZ_VARIANTS;
use mz_ore::codegen::CodegenBuf;
use mz_ore::str::StrExt;
use uncased::UncasedStr;

const DEFAULT_TZNAMES: &str = "tznames/Default";

enum TimezoneAbbrevSpec<'a> {
    FixedOffset { utc_offset_secs: i32, is_dst: bool },
    Tz(&'a str),
}

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("Cannot read OUT_DIR env var")?);

    // Build protobufs.
    {
        env::set_var("PROTOC", protobuf_src::protoc());

        prost_build::Config::new()
            .btree_map(["."])
            .extern_path(".mz_proto", "::mz_proto")
            .compile_protos(&["pgtz/src/timezone.proto"], &[".."])?;
    }

    // Convert the default PostgreSQL timezone abbreviation file into a Rust
    // constants, one for each abbrevation in the file, and the SQL definition
    // of the `pg_timezone_abbrevs` view.
    //
    // See: https://www.postgresql.org/docs/16/datetime-config-files.html
    {
        let mut sql_buf = CodegenBuf::new();
        let mut rust_buf = CodegenBuf::new();
        let mut phf_map = phf_codegen::Map::new();

        sql_buf.writeln("VALUES");

        let tznames = fs::read_to_string(DEFAULT_TZNAMES)?;
        let mut emitted_abbrev = false;
        for (i, line) in tznames.lines().enumerate() {
            let pieces = line.split_ascii_whitespace().collect::<Vec<_>>();

            if let Some(p) = pieces.first() {
                if p.starts_with('#') {
                    // Comment line.
                    continue;
                }
            } else if pieces.len() == 0 {
                // Empty line.
                continue;
            } else if pieces.len() < 2 {
                bail!("line {}: did not find at least two fields", i + 1);
            }

            let abbrev = pieces[0];
            let spec = match pieces[1].parse::<i32>() {
                Ok(utc_offset_secs) => TimezoneAbbrevSpec::FixedOffset {
                    utc_offset_secs,
                    is_dst: pieces.get(2) == Some(&"D"),
                },
                Err(_) => TimezoneAbbrevSpec::Tz(pieces[1]),
            };

            rust_buf.write_block(
                format!("pub const {abbrev}: TimezoneAbbrev = TimezoneAbbrev"),
                |rust_buf| {
                    rust_buf.writeln(format!("abbrev: {},", abbrev.quoted()));
                    match &spec {
                        TimezoneAbbrevSpec::FixedOffset {
                            utc_offset_secs,
                            is_dst,
                        } => {
                            rust_buf.write_block(
                                "spec: TimezoneAbbrevSpec::FixedOffset",
                                |rust_buf| {
                                    rust_buf.writeln(format!(
                                        "offset: make_fixed_offset({utc_offset_secs}),"
                                    ));
                                    rust_buf.writeln(format!("is_dst: {is_dst},"));
                                },
                            );
                        }
                        TimezoneAbbrevSpec::Tz(name) => {
                            let name = name.replace('/', "__");
                            rust_buf.writeln(format!("spec: TimezoneAbbrevSpec::Tz(Tz::{name})"));
                        }
                    }
                },
            );
            rust_buf.writeln(";");

            let (sql_utc_offset, sql_is_dst) = match &spec {
                TimezoneAbbrevSpec::FixedOffset {
                    utc_offset_secs,
                    is_dst,
                } => {
                    let utc_offset = format!("interval '{utc_offset_secs} seconds'");
                    let is_dst = is_dst.to_string();
                    (utc_offset, is_dst)
                }
                TimezoneAbbrevSpec::Tz(name) => {
                    let utc_offset = format!("timezone_offset('{name}', now()).base_utc_offset + timezone_offset('{name}', now()).dst_offset");
                    let is_dst =
                        format!("timezone_offset('{name}', now()).dst_offset <> interval '0'");
                    (utc_offset, is_dst)
                }
            };
            if emitted_abbrev {
                sql_buf.writeln(",");
            }
            sql_buf.writeln(format!("('{abbrev}', {sql_utc_offset}, {sql_is_dst})"));

            phf_map.entry(UncasedStr::new(abbrev), abbrev);

            emitted_abbrev = true;
        }

        sql_buf.end_line();

        rust_buf.writeln(format!(
            "pub static TIMEZONE_ABBREVS: phf::Map<&'static UncasedStr, TimezoneAbbrev> = {};",
            phf_map.build(),
        ));

        fs::write(out_dir.join("abbrev.gen.sql"), sql_buf.into_string())?;
        fs::write(out_dir.join("abbrev.gen.rs"), rust_buf.into_string())?;
    }

    // Convert chrono-tz's list of timezones into the SQL definition of the
    // pg_timezone_names view.
    {
        let mut sql_buf = CodegenBuf::new();

        sql_buf.writeln("VALUES");

        for (i, tz) in TZ_VARIANTS.iter().enumerate() {
            let name = tz.name();
            if i > 0 {
                sql_buf.writeln(",");
            }
            sql_buf.write("(");
            sql_buf.write(format!("'{name}',"));
            sql_buf.write(format!("timezone_offset('{name}', now()).abbrev,"));
            sql_buf.write(format!("timezone_offset('{name}', now()).base_utc_offset + timezone_offset('{name}', now()).dst_offset,"));
            sql_buf.write(format!(
                "timezone_offset('{name}', now()).dst_offset <> interval '0'"
            ));
            sql_buf.write(")");
        }

        fs::write(out_dir.join("timezone.gen.sql"), sql_buf.into_string())?;
    }

    Ok(())
}