== 1. toolchain ==
  pinned: 1.97.1
  active: rustc 1.97.1 (8bab26f4f 2026-07-14)
  source: 1.97.1-x86_64-unknown-linux-gnu (overridden by '/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/rust-toolchain.toml')
  pin HONORED: active toolchain is sourced from the pin file
== 2. tree receipt ==
  HEAD: 1689708d2033aa66180a76012a1dff274780fcfb
  TREE: CLEAN
== 3. paths census ==
  census OK (93 files within the wall, 59 in the deliverable crate family)
== 4a. clippy ==
   Compiling proc-macro2 v1.0.106
   Compiling unicode-ident v1.0.24
   Compiling quote v1.0.45
   Compiling libc v0.2.186
    Checking cfg-if v1.0.4
    Checking allocator-api2 v0.2.21
    Checking equivalent v1.0.2
    Checking foldhash v0.2.0
   Compiling serde_core v1.0.228
    Checking memchr v2.8.1
    Checking itoa v1.0.18
    Checking once_cell v1.21.4
   Compiling serde v1.0.228
    Checking smallvec v1.15.1
    Checking log v0.4.32
    Checking hashbrown v0.17.1
   Compiling zmij v1.0.21
   Compiling serde_json v1.0.150
   Compiling parking_lot_core v0.9.12
    Checking scopeguard v1.2.0
   Compiling thiserror v2.0.18
    Checking lock_api v0.4.14
    Checking pin-project-lite v0.2.17
   Compiling shlex v2.0.1
    Checking rand_core v0.10.1
    Checking indexmap v2.14.0
   Compiling syn v2.0.117
   Compiling find-msvc-tools v0.1.9
   Compiling autocfg v1.5.1
   Compiling getrandom v0.4.2
    Checking typenum v1.20.1
   Compiling version_check v0.9.5
   Compiling num-traits v0.2.19
    Checking zeroize v1.8.2
    Checking bitflags v2.13.0
    Checking bytes v1.11.1
    Checking iana-time-zone v0.1.65
    Checking parking_lot v0.12.5
    Checking errno v0.3.14
   Compiling jobserver v0.1.34
    Checking signal-hook-registry v1.4.8
    Checking mio v1.2.1
    Checking socket2 v0.6.4
    Checking cpufeatures v0.3.0
   Compiling rustix v1.1.4
    Checking linux-raw-sys v0.12.1
   Compiling heck v0.5.0
   Compiling cc v1.2.63
    Checking uuid v1.23.4
   Compiling termcolor v1.4.1
    Checking hybrid-array v0.4.12
   Compiling zerocopy v0.8.50
    Checking hashbrown v0.16.1
    Checking tracing-core v0.1.36
   Compiling getrandom v0.3.4
   Compiling pkg-config v0.3.33
    Checking percent-encoding v2.3.2
    Checking crypto-common v0.2.2
    Checking cmov v0.5.4
   Compiling anyhow v1.0.104
    Checking stable_deref_trait v1.2.1
    Checking ctutils v0.4.2
    Checking block-buffer v0.12.0
    Checking bumpalo v3.20.3
    Checking const-oid v0.10.2
   Compiling zstd-safe v7.2.4
    Checking futures-core v0.3.32
   Compiling ref-cast v1.0.25
   Compiling crc32fast v1.5.0
    Checking fnv v1.0.7
    Checking simd-adler32 v0.3.9
    Checking digest v0.11.3
    Checking data-encoding v2.11.0
    Checking futures-sink v0.3.32
    Checking adler2 v2.0.1
   Compiling zstd-sys v2.0.16+zstd.1.5.7
    Checking litemap v0.8.2
    Checking writeable v0.6.3
    Checking miniz_oxide v0.8.9
    Checking fastrand v2.4.1
    Checking utf8_iter v1.0.4
   Compiling icu_properties_data v2.2.0
   Compiling icu_normalizer_data v2.2.0
    Checking zlib-rs v0.6.3
    Checking regex-syntax v0.8.10
    Checking constant_time_eq v0.4.2
    Checking slab v0.4.12
    Checking futures-channel v0.3.32
    Checking sha2 v0.11.0
    Checking num-integer v0.1.46
    Checking aho-corasick v1.1.4
   Compiling synstructure v0.13.2
    Checking flate2 v1.1.9
   Compiling ahash v0.8.12
    Checking futures-task v0.3.32
    Checking regex-automata v0.4.18
    Checking futures-io v0.3.32
    Checking lazy_static v1.5.0
    Checking num-bigint v0.4.6
    Checking num-iter v0.1.45
    Checking num-complex v0.4.6
    Checking bit-vec v0.8.0
    Checking borrow-or-share v0.2.4
    Checking bit-set v0.8.0
   Compiling unicode-general-category v1.1.0
    Checking micromap v0.3.0
   Compiling cranelift-srcgen v0.134.3
   Compiling libm v0.2.16
    Checking tempfile v3.27.0
   Compiling cranelift-assembler-x64-meta v0.134.3
    Checking vsimd v0.8.0
    Checking num-rational v0.4.2
   Compiling serde_derive v1.0.228
   Compiling thiserror-impl v2.0.18
   Compiling tokio-macros v2.7.0
   Compiling ts-rs-macros v12.0.1
   Compiling tracing-attributes v0.1.31
    Checking tokio v1.52.3
   Compiling zerofrom-derive v0.1.7
   Compiling yoke-derive v0.8.2
    Checking tracing v0.1.44
   Compiling zerovec-derive v0.11.3
   Compiling displaydoc v0.2.6
   Compiling ref-cast-impl v1.0.25
   Compiling futures-macro v0.3.32
    Checking zerofrom v0.1.8
    Checking num v0.4.3
    Checking fraction v0.15.4
    Checking yoke v0.8.3
   Compiling async-trait v0.1.89
    Checking zerotrie v0.2.4
    Checking zerovec v0.11.6
   Compiling wasmtime-internal-core v47.0.3
    Checking num-cmp v0.1.0
    Checking futures-util v0.3.32
    Checking bytecount v0.6.9
    Checking outref v0.5.2
    Checking uuid-simd v0.8.0
    Checking regex v1.12.3
    Checking tinystr v0.8.3
    Checking potential_utf v0.1.5
    Checking icu_collections v2.2.0
    Checking icu_locale_core v2.2.0
    Checking chrono v0.4.45
    Checking fluent-uri v0.4.1
    Checking email_address v0.2.9
    Checking icu_provider v2.2.0
   Compiling cranelift-isle v0.134.3
    Checking icu_normalizer v2.2.0
    Checking icu_properties v2.2.0
   Compiling cranelift-codegen-shared v0.134.3
   Compiling crossbeam-utils v0.8.21
   Compiling cranelift-codegen-meta v0.134.3
    Checking cranelift-bitset v0.134.3
   Compiling cranelift-assembler-x64 v0.134.3
    Checking ts-rs v12.0.1
   Compiling target-lexicon v0.13.5
    Checking aion-core v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-core)
    Checking cranelift-entity v0.134.3
    Checking idna_adapter v1.2.2
    Checking idna v1.1.0
    Checking referencing v0.46.6
    Checking fancy-regex v0.18.0
    Checking rustc-hash v2.1.2
    Checking arbitrary v1.4.2
    Checking regalloc2 v0.15.1
    Checking cranelift-bforest v0.134.3
    Checking jsonschema v0.46.6
    Checking cranelift-control v0.134.3
    Checking sha1 v0.11.0
    Checking gimli v0.33.0
    Checking winnow v1.0.3
    Checking toml_parser v1.1.2+spec-1.1.0
    Checking inout v0.2.2
    Checking serde_spanned v1.1.1
   Compiling cranelift-codegen v0.134.3
    Checking toml_writer v1.1.1+spec-1.1.0
    Checking bitflags v1.3.2
    Checking powerfmt v0.2.0
    Checking cipher v0.5.2
    Checking hmac v0.13.0
    Checking deranged v0.5.8
    Checking toml_datetime v1.1.1+spec-1.1.0
    Checking libbz2-rs-sys v0.2.5
    Checking cpubits v0.1.1
   Compiling camino v1.2.2
    Checking time-core v0.1.8
    Checking num-conv v0.2.2
    Checking bzip2 v0.6.1
    Checking aes v0.9.1
    Checking toml v1.1.2+spec-1.1.0
    Checking pbkdf2 v0.13.0
    Checking time v0.3.47
    Checking lzma-rust2 v0.16.4
    Checking zopfli v0.8.3
    Checking chacha20 v0.10.0
    Checking deflate64 v0.1.12
   Compiling io-uring v0.7.12
    Checking typed-path v0.12.3
    Checking ppmd-rust v1.4.0
    Checking rand v0.10.2
    Checking region v3.0.2
    Checking crossbeam-epoch v0.9.18
    Checking crossbeam-channel v0.5.15
    Checking wasmtime-internal-jit-icache-coherence v47.0.3
    Checking memmap2 v0.9.11
    Checking hashbrown v0.14.5
    Checking crossbeam-deque v0.8.6
    Checking gleam-types v0.4.4
    Checking crossbeam-queue v0.3.12
    Checking futures-executor v0.3.32
    Checking num_cpus v1.17.0
    Checking unicode-segmentation v1.13.3
    Checking md5 v0.8.0
    Checking dashmap v6.2.1
    Checking web-time v1.1.0
    Checking base64 v0.23.1
    Checking futures v0.3.32
    Checking tokio-util v0.7.18
    Checking ppv-lite86 v0.2.21
   Compiling blake3 v1.8.5
    Checking arrayref v0.3.9
    Checking arrayvec v0.7.7
    Checking lru v0.18.2
   Compiling generic-array v0.14.7
    Checking http v1.4.1
   Compiling cmake v0.1.58
   Compiling dunce v1.0.5
   Compiling fs_extra v1.3.0
    Checking rustls-pki-types v1.14.1
   Compiling aws-lc-sys v0.41.0
   Compiling httparse v1.10.1
   Compiling aws-lc-rs v1.17.0
    Checking http-body v1.0.1
    Checking form_urlencoded v1.2.2
   Compiling rustls v0.23.40
    Checking untrusted v0.9.0
    Checking base64 v0.22.1
    Checking ipnet v2.12.0
   Compiling either v1.16.0
    Checking block-buffer v0.10.4
    Checking crypto-common v0.1.7
    Checking tower-service v0.3.3
    Checking subtle v2.6.1
    Checking digest v0.10.7
   Compiling itertools v0.14.0
    Checking atomic-waker v1.1.2
    Checking try-lock v0.2.5
    Checking cpufeatures v0.2.17
    Checking want v0.3.1
    Checking h2 v0.4.14
    Checking httpdate v1.0.3
    Checking sync_wrapper v1.0.2
    Checking rand_core v0.9.5
   Compiling prettyplease v0.2.37
   Compiling pulldown-cmark v0.13.4
    Checking tower-layer v0.3.3
    Checking rand_chacha v0.9.0
   Compiling foldhash v0.1.5
   Compiling prost-derive v0.14.3
   Compiling unicase v2.9.0
   Compiling prost v0.14.3
    Checking hyper v1.10.1
    Checking rand v0.9.4
   Compiling hashbrown v0.15.5
    Checking hyper-util v0.1.20
    Checking zstd v0.13.3
    Checking zip v8.6.0
   Compiling beamr v0.20.0
    Checking tower v0.5.3
    Checking sha1 v0.10.6
    Checking aion-store v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-store)
    Checking http-body-util v0.1.3
    Checking webpki-roots v1.0.7
   Compiling fixedbitset v0.5.7
    Checking aion-package v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-package)
   Compiling petgraph v0.8.3
    Checking webpki-roots v0.26.11
   Compiling pulldown-cmark-to-cmark v22.0.0
   Compiling prost-types v0.14.3
    Checking ryu v1.0.23
    Checking mime v0.3.17
   Compiling multimap v0.10.1
    Checking axum-core v0.5.6
    Checking serde_urlencoded v0.7.1
   Compiling tonic-build v0.14.6
   Compiling prost-build v0.14.3
    Checking tokio-stream v0.1.18
   Compiling pin-project-internal v1.1.13
    Checking serde_path_to_error v0.1.20
   Compiling protoc-bin-vendored-linux-s390_64 v3.2.0
   Compiling protoc-bin-vendored-macos-x86_64 v3.2.0
   Compiling protoc-bin-vendored-linux-aarch_64 v3.2.0
   Compiling protoc-bin-vendored-linux-x86_32 v3.2.0
    Checking matchit v0.8.4
   Compiling protoc-bin-vendored-win32 v3.2.0
   Compiling protoc-bin-vendored-linux-x86_64 v3.2.0
   Compiling protoc-bin-vendored-macos-aarch_64 v3.2.0
   Compiling protoc-bin-vendored-linux-ppcle_64 v3.2.0
   Compiling tonic-prost-build v0.14.5
   Compiling protoc-bin-vendored v3.2.0
    Checking hyper-timeout v0.5.2
    Checking aion-integrations v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-integrations)
    Checking pin-project v1.1.13
   Compiling strum_macros v0.28.0
   Compiling serde_derive_internals v0.29.1
   Compiling cfg_aliases v0.2.1
   Compiling nix v0.30.1
    Checking dyn-clone v1.0.20
   Compiling aion-proto-generated v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-proto-generated)
   Compiling nom v5.1.3
    Checking referencing v0.49.9
    Checking jsonschema-value v0.49.9
    Checking fancy-regex v0.19.0
    Checking jsonschema-regex v0.49.9
   Compiling schemars_derive v1.2.1
    Checking tungstenite v0.30.0
    Checking rand_chacha v0.10.0
    Checking liminal-protocol v0.7.1
    Checking spin v0.12.3
    Checking hostname v0.4.2
    Checking tracing-serde v0.2.0
    Checking matchers v0.2.0
    Checking sharded-slab v0.1.7
    Checking tracing-log v0.2.0
    Checking thread_local v1.1.9
    Checking nu-ansi-term v0.50.3
    Checking fs4 v1.1.0
    Checking tracing-subscriber v0.3.23
    Checking strum v0.28.0
    Checking utf8parse v0.2.2
    Checking jsonschema v0.49.9
    Checking saffron v0.1.0
    Checking anstyle-parse v1.0.0
    Checking anstyle-query v1.1.5
    Checking colorchoice v1.0.5
    Checking is_terminal_polyfill v1.70.2
    Checking anstyle v1.0.14
    Checking schemars v1.2.1
    Checking clap_lex v1.1.0
    Checking anstream v1.0.0
    Checking strsim v0.11.1
   Compiling clap_derive v4.6.1
    Checking clap_builder v4.6.0
    Checking url v2.5.8
   Compiling io-lifetimes v3.0.1
   Compiling io-lifetimes v2.0.4
   Compiling io-extras v0.19.0
   Compiling thiserror v1.0.69
   Compiling thiserror-impl v1.0.69
    Checking same-file v1.0.6
    Checking clap v4.6.1
   Compiling cap-primitives v4.0.2
   Compiling sha2 v0.10.9
   Compiling walkdir v2.5.0
    Checking fs-set-times v0.20.3
    Checking aion-toolchain v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-toolchain)
    Checking rustix-linux-procfs v0.1.1
    Checking ambient-authority v0.0.2
   Compiling cap-std v4.0.2
   Compiling signal-hook v0.4.4
    Checking maybe-owned v0.3.4
   Compiling protobuf v3.7.2
    Checking cranelift-module v0.134.3
    Checking cranelift-native v0.134.3
    Checking cranelift-frontend v0.134.3
   Compiling rust-embed-utils v8.11.0
    Checking cranelift-jit v0.134.3
    Checking aion-mcp v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-mcp)
   Compiling prometheus v0.14.0
   Compiling cap-fs-ext v4.0.2
    Checking protobuf-support v3.7.2
   Compiling signal-hook v0.3.18
    Checking openssl-probe v0.2.1
    Checking rustls-native-certs v0.8.4
   Compiling rust-embed-impl v8.11.0
    Checking tower-http v0.6.11
   Compiling aion-server v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-server)
   Compiling ident_case v1.0.1
   Compiling unicode-xid v0.2.6
    Checking rustls-pemfile v2.2.0
    Checking fluent-uri v0.1.4
    Checking rust-embed v8.11.0
   Compiling darling_core v0.23.0
   Compiling serde_repr v0.1.20
    Checking lsp-server v0.8.0
   Compiling convert_case v0.10.0
    Checking aion-integration-norn v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-integration-norn)
    Checking lsp-types v0.97.0
    Checking inotify-sys v0.1.5
    Checking inotify v0.11.2
   Compiling derive_more-impl v2.1.1
    Checking toml_datetime v0.7.5+spec-1.1.0
    Checking notify-types v2.1.0
    Checking winnow v0.7.15
    Checking notify v8.2.0
    Checking aion-fleet-plan v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-fleet-plan)
    Checking toml_edit v0.23.10+spec-1.0.0
    Checking aion-integration-cli v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-integration-cli)
    Checking derive_more v2.1.1
   Compiling darling_macro v0.23.0
   Compiling darling v0.23.0
   Compiling serde_with_macros v3.21.0
    Checking serde_with v3.21.0
    Checking haematite v0.11.0
    Checking aion-awl v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-awl)
    Checking aion-rs v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion)
    Checking agent-client-protocol-schema v1.5.0
    Checking liminal-rs v0.7.0
    Checking aion-store-haematite v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-store-haematite)
    Checking xtask v0.0.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/xtask)
    Checking aion-darwin-acl v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-darwin-acl)
    Checking aion-integration-acp v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-integration-acp)
    Checking aion-awl-package v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-awl-package)
    Checking aion-awl-lsp v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-awl-lsp)
    Checking liminal-sdk v0.9.0
    Checking liminal-server v0.10.0
    Checking million-processes v0.1.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/benchmarks/million-processes)
    Checking rustls-webpki v0.103.13
    Checking tokio-rustls v0.26.4
    Checking tungstenite v0.29.0
    Checking rustls-platform-verifier v0.7.0
    Checking hyper-rustls v0.27.9
    Checking reqwest v0.13.4
    Checking tokio-tungstenite v0.29.0
    Checking axum v0.8.9
    Checking tonic v0.14.6
    Checking tonic-prost v0.14.5
    Checking aion-proto v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-proto)
    Checking aion-worker v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-worker)
    Checking aion-client v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-client)
    Checking aion-cli v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-cli)
    Finished `dev` profile [unoptimized] target(s) in 1m 18s
  clippy clean
== 4b. new-test control ==
  new test annotations in diff: 71
== 4c. suite ==
  suite names ~5472 test(s)
    Finished `test` profile [unoptimized] target(s) in 0.31s
────────────
 Nextest run ID a97abd63-03bd-4d5f-bf1a-9966c6321c43 with nextest profile: default
    Starting 5472 tests across 282 binaries
        PASS [   0.005s] (   1/5472) aion-awl ast::command_tests::a_dangling_dollar_at_the_end_is_refused
        PASS [   0.005s] (   2/5472) aion-awl ast::command_tests::a_command_without_a_dollar_has_no_references
        PASS [   0.006s] (   3/5472) aion-awl ast::command_tests::a_bare_reference_reports_its_name_offset_and_length
        PASS [   0.005s] (   4/5472) aion-awl ast::command_tests::a_reference_in_program_position_is_reported_inside_the_program_word
        PASS [   0.006s] (   5/5472) aion-awl ast::command_tests::a_dollar_inside_single_quotes_is_literal
        PASS [   0.006s] (   6/5472) aion-awl ast::command_tests::a_dollar_inside_double_quotes_is_a_reference
        PASS [   0.006s] (   7/5472) aion-awl ast::command_tests::a_brace_reference_holding_a_non_name_is_refused
        PASS [   0.006s] (   8/5472) aion-awl ast::command_tests::a_unicode_name_is_refused_rather_than_split
        PASS [   0.006s] (   9/5472) aion-awl ast::command_tests::a_brace_reference_holding_spaces_is_refused
        PASS [   0.007s] (  10/5472) aion-awl ast::command_tests::a_reference_ending_the_command_is_measured_to_the_end
        PASS [   0.007s] (  11/5472) aion-awl ast::command_tests::a_doubled_dollar_before_a_brace_stays_literal
        PASS [   0.007s] (  12/5472) aion-awl ast::command_tests::a_dollar_before_a_space_is_refused
        PASS [   0.007s] (  13/5472) aion-awl ast::command_tests::adjacent_references_are_both_found
        PASS [   0.008s] (  14/5472) aion-awl ast::command_tests::a_name_may_carry_digits_and_underscores
        PASS [   0.008s] (  15/5472) aion-awl ast::command_tests::a_doubled_dollar_is_a_literal_dollar_and_never_a_reference
        PASS [   0.008s] (  16/5472) aion-awl ast::command_tests::a_dollar_before_a_hyphen_is_refused
        PASS [   0.006s] (  17/5472) aion-awl ast::command_tests::an_unterminated_brace_reference_is_refused
        PASS [   0.006s] (  18/5472) aion-awl ast::command_tests::an_uppercase_shell_variable_is_refused_with_the_escape_hint
        PASS [   0.006s] (  19/5472) aion-awl ast::command_tests::the_empty_command_scans_to_nothing
        PASS [   0.006s] (  20/5472) aion-awl ast::command_tests::parameters_are_sorted_and_deduplicated
        PASS [   0.006s] (  21/5472) aion-awl ast::command_tests::multibyte_text_before_a_reference_keeps_byte_offsets_honest
        PASS [   0.006s] (  22/5472) aion-awl ast::command_tests::an_empty_brace_reference_is_refused
        PASS [   0.006s] (  23/5472) aion-awl ast::command_tests::braces_delimit_a_reference_that_abuts_more_name_characters
        PASS [   0.006s] (  24/5472) aion-awl ast::command_tests::an_unterminated_double_quote_is_refused
        PASS [   0.006s] (  25/5472) aion-awl ast::command_tests::an_unterminated_single_quote_is_refused
        PASS [   0.006s] (  26/5472) aion-awl checker::harness::tests::a_duplicate_setting_is_refused_at_the_parse_because_neither_reading_is_honest
        PASS [   0.006s] (  27/5472) aion-awl ast::command_tests::the_program_word_ends_at_the_first_unquoted_whitespace
        PASS [   0.007s] (  28/5472) aion-awl checker::harness::tests::a_norn_section_is_never_asked_for_an_acp_setting
        PASS [   0.006s] (  29/5472) aion-awl checker::harness::tests::an_env_pass_entry_carrying_a_value_is_refused_rather_than_split
        PASS [   0.007s] (  30/5472) aion-awl checker::harness::tests::a_setting_for_the_other_kind_is_refused_by_name_in_both_directions
        PASS [   0.006s] (  31/5472) aion-awl command::cause::cause_tests::every_layer_files_at_least_one_cause_and_every_cause_has_a_layer
        PASS [   0.006s] (  32/5472) aion-awl command::cause::cause_tests::layer_wire_strings_are_distinct
        PASS [   0.006s] (  33/5472) aion-awl command::cause::cause_tests::every_wire_string_is_distinct_and_shaped_like_a_cause_code
        PASS [   0.006s] (  34/5472) aion-awl command::cause::cause_tests::the_inventory_carries_every_variant_exactly_once
        PASS [   0.005s] (  35/5472) aion-awl command::derived::derived_tests::every_shape_names_its_derivation
        PASS [   0.006s] (  36/5472) aion-awl command::cause::cause_tests::the_vocabulary_carries_no_catch_all_code
        PASS [   0.007s] (  37/5472) aion-awl checker::harness::tests::an_unknown_kind_suppresses_the_per_kind_demands_rather_than_guessing_one
        PASS [   0.005s] (  38/5472) aion-awl command::error::error_tests::a_constructor_fed_a_cause_from_another_layer_returns_a_typed_error
        PASS [   0.009s] (  39/5472) aion-awl checker::harness::tests::an_unknown_setting_is_refused_naming_what_the_section_carries
        PASS [   0.006s] (  40/5472) aion-awl command::derived::derived_tests::the_three_derived_shapes_are_caught
        PASS [   0.007s] (  41/5472) aion-awl command::derived::derived_tests::ordinary_text_is_left_alone
        PASS [   0.009s] (  42/5472) aion-awl checker::harness::tests::every_required_norn_field_is_named_when_it_is_the_one_removed
        PASS [   0.010s] (  43/5472) aion-awl command::cause::cause_tests::a_cause_round_trips_through_its_wire_string
        PASS [   0.006s] (  44/5472) aion-awl command::error::error_tests::a_rendered_refusal_carries_its_observed_bytes_and_its_cause
        PASS [   0.006s] (  45/5472) aion-awl command::error::error_tests::the_internal_door_files_every_cause_at_its_own_layer
        PASS [   0.012s] (  46/5472) aion-awl checker::harness::tests::every_required_acp_field_is_named_when_it_is_the_one_removed
        PASS [   0.006s] (  47/5472) aion-awl command::error::error_tests::every_cause_constructs_at_its_own_layer_and_nowhere_else
        PASS [   0.006s] (  48/5472) aion-awl command::error::error_tests::a_constructor_fed_an_empty_diagnostic_returns_a_typed_error
        PASS [   0.006s] (  49/5472) aion-awl command::error::error_tests::the_internal_door_never_produces_a_silent_refusal
        PASS [   0.005s] (  50/5472) aion-awl command::fill::fill_tests::a_hole_span_narrows_into_the_literal_when_it_can
        PASS [   0.006s] (  51/5472) aion-awl command::fill::fill_tests::a_backtick_is_refused_at_the_parse_layer_with_its_own_cause
        PASS [   0.005s] (  52/5472) aion-awl command::fill::fill_tests::a_single_brace_is_literal_text
        PASS [   0.006s] (  53/5472) aion-awl command::fill::fill_tests::a_sole_hole_is_recognised_as_such
        PASS [   0.005s] (  54/5472) aion-awl command::fill::fill_tests::a_plain_literal_is_one_segment
        PASS [   0.006s] (  55/5472) aion-awl command::fill::fill_tests::a_mixed_fill_keeps_its_segments_in_source_order
        PASS [   0.006s] (  56/5472) aion-awl command::fill::fill_tests::an_empty_interpolation_is_refused
        PASS [   0.008s] (  57/5472) aion-awl command::fill::fill_tests::an_interpolation_that_is_not_a_bare_parameter_name_is_refused
        PASS [   0.007s] (  58/5472) aion-awl command::fill::fill_tests::an_unterminated_interpolation_is_refused
        PASS [   0.005s] (  59/5472) aion-awl guide::grammar::grammar_tests::the_table_offers_no_word_the_lexer_does_not_name
        PASS [   0.005s] (  60/5472) aion-awl guide::grammar::grammar_tests::wire_identifiers_are_distinct_and_prose_is_one_finished_line
        PASS [   0.006s] (  61/5472) aion-awl guide::grammar::grammar_tests::the_canonical_words_land_in_their_forms
        PASS [   0.005s] (  62/5472) aion-awl guide::lookup::guide_tests::a_citation_names_the_reference_and_the_section
        PASS [   0.008s] (  63/5472) aion-awl guide::grammar::grammar_tests::every_form_files_at_least_one_word
        PASS [   0.008s] (  64/5472) aion-awl guide::grammar::grammar_tests::offered_words_compose_keywords_then_positional_words
        PASS [   0.007s] (  65/5472) aion-awl guide::grammar::grammar_tests::the_all_inventories_carry_every_variant
        PASS [   0.008s] (  66/5472) aion-awl command::fill::fill_tests::the_same_fill_parses_identically_every_time
        PASS [   0.008s] (  67/5472) aion-awl guide::grammar::grammar_tests::positions_are_read_from_both_halves_of_the_table_in_order
        PASS [   0.008s] (  68/5472) aion-awl guide::grammar::grammar_tests::every_word_of_the_vocabulary_stands_somewhere
        PASS [   0.009s] (  69/5472) aion-awl guide::grammar::grammar_tests::every_derived_form_is_honest_against_the_table
        PASS [   0.005s] (  70/5472) aion-awl guide::lookup::guide_tests::every_keyword_entry_names_the_word_the_lexer_produces
        PASS [   0.005s] (  71/5472) aion-awl guide::lookup::guide_tests::every_positional_word_has_an_entry_recorded_as_positional
        PASS [   0.005s] (  72/5472) aion-awl guide::lookup::guide_tests::every_summary_is_one_rendered_line
        PASS [   0.005s] (  73/5472) aion-awl guide::lookup::guide_tests::the_example_word_matcher_reads_tokens_not_text
        PASS [   0.005s] (  74/5472) aion-awl guide::lookup::guide_tests::every_word_of_the_language_is_reachable_by_name
        PASS [   0.006s] (  75/5472) aion-awl guide::lookup::guide_tests::the_two_inventories_do_not_overlap
        PASS [   0.005s] (  76/5472) aion-awl harness_config::tests::a_relative_norn_binary_is_named_at_the_launch_seam_not_carried_through
        PASS [   0.006s] (  77/5472) aion-awl guide::lookup::guide_tests::the_inventory_is_complete_and_carries_no_duplicate
        PASS [   0.006s] (  78/5472) aion-awl harness_config::tests::a_missing_norn_cwd_is_named_at_the_launch_seam
        PASS [   0.006s] (  79/5472) aion-awl harness_config::tests::a_relative_acp_command_is_named_by_its_own_field
        PASS [   0.007s] (  80/5472) aion-awl harness_config::tests::a_misplaced_workspace_root_in_cwd_is_refused_at_the_seam
        PASS [   0.033s] (  81/5472) aion-awl checker::harness::tests::an_acp_section_is_complete_without_any_norn_setting
        PASS [   0.005s] (  82/5472) aion-awl harness_config::tests::an_omitted_norn_binary_is_absence_and_not_a_refusal
        PASS [   0.006s] (  83/5472) aion-awl harness_config::tests::an_absolute_norn_binary_passes_the_same_net
        PASS [   0.006s] (  84/5472) aion-awl harness_config::tests::a_workspace_rooted_cwd_reaches_the_launch_seam_unexpanded
        PASS [   0.005s] (  85/5472) aion-awl lexer::grammar_vocabulary_tests::the_specifically_typed_words_are_still_covered
        PASS [   0.005s] (  86/5472) aion-awl lexer::tokens_tests::every_keyword_in_the_inventory_round_trips_and_is_distinct
        PASS [   0.005s] (  87/5472) aion-awl lexer::grammar_vocabulary_tests::the_grammar_colours_nothing_the_lexer_cannot_account_for
        PASS [   0.006s] (  88/5472) aion-awl lexer::grammar_vocabulary_tests::the_bindable_positional_words_are_deliberately_left_plain
        PASS [   0.007s] (  89/5472) aion-awl lexer::grammar_vocabulary_tests::every_reserved_word_is_known_to_the_editor_grammar
        PASS [   0.006s] (  90/5472) aion-awl lexer::tokens_tests::keyword_inventory_covers_every_variant
        PASS [   0.037s] (  91/5472) aion-awl checker::harness::tests::a_norn_section_is_complete_without_command_permission_or_exit_grace
        PASS [   0.007s] (  92/5472) aion-awl lexer::grammar_vocabulary_tests::the_coloured_exception_is_a_genuine_positional_word
        PASS [   0.006s] (  93/5472) aion-awl lexer::tokens_tests::the_positional_words_are_named_without_being_reserved
        PASS [   0.021s] (  94/5472) aion-awl guide::lookup::guide_tests::every_entry_example_is_a_complete_document_using_its_word
        PASS [   0.017s] (  95/5472) aion-awl guide::lookup::guide_tests::the_embedded_reference_carries_the_cited_sections
        PASS [   0.013s] (  96/5472) aion-awl mir::agent_control_tests::the_control_pair_differs_only_by_the_marker
        PASS [   0.013s] (  97/5472) aion-awl mir::agent_control_tests::agent_marked_and_unmarked_lower_identically
        PASS [   0.026s] (  98/5472) aion-awl mir::deferred_tests::a_substep_visit_bound_refuses_rather_than_going_unenforced
        PASS [   0.028s] (  99/5472) aion-awl mir::codec_tests::required_recursive_default_is_refused_on_both_sides
        PASS [   0.027s] ( 100/5472) aion-awl mir::deferred_tests::a_substeps_failure_edge_keeps_its_routed_outcome_out_of_the_guard
        PASS [   0.028s] ( 101/5472) aion-awl mir::deferred_tests::a_subflow_outcome_payload_is_wrapped_for_an_optional_outcome
        PASS [   0.029s] ( 102/5472) aion-awl mir::codec_tests::colliding_codec_stems_are_refused_not_cross_wired
        PASS [   0.030s] ( 103/5472) aion-awl mir::child_fork_tests::parallel_child_fork_is_spawn_all_then_ordered_await_with_emitter_parity
        PASS [   0.028s] ( 104/5472) aion-awl mir::deferred_tests::a_substep_chain_hands_off_and_closes_on_the_parents_arms
        PASS [   0.031s] ( 105/5472) aion-awl mir::deferred_tests::a_parallel_layer_dispatches_through_one_workflow_all
        PASS [   0.032s] ( 106/5472) aion-awl mir::child_fork_tests::sequential_child_fork_matches_reference_fold_order
        PASS [   0.033s] ( 107/5472) aion-awl mir::child_fork_tests::child_spawn_keeps_identity_and_outcome_envelope_codec
        PASS [   0.028s] ( 108/5472) aion-awl mir::deferred_tests::bc4_zero_step_workflow_refuses_no_start_region
        PASS [   0.041s] ( 109/5472) aion-awl mir::deferred_tests::a_degraded_parallel_layer_is_marked_on_its_first_member_only
        PASS [   0.031s] ( 110/5472) aion-awl mir::deferred_tests::each_subflow_keeps_its_own_binding_environment
        PASS [   0.040s] ( 111/5472) aion-awl mir::deferred_tests::a_value_payload_is_wrapped_for_an_optional_outcome
        PASS [   0.041s] ( 112/5472) aion-awl mir::deferred_tests::an_advisory_call_keeps_its_layer_in_written_order
        PASS [   0.006s] ( 113/5472) aion-awl mir::fork_tests::collection_child_fork_config_refuses_with_the_child_class
        PASS [   0.029s] ( 114/5472) aion-awl mir::deferred_tests::nested_handlers_on_failure_now_lowers
        PASS [   0.029s] ( 115/5472) aion-awl mir::deferred_tests::guard_optional_wait_now_lowers
        PASS [   0.028s] ( 116/5472) aion-awl mir::deferred_tests::post_join_combinator_now_lowers
        PASS [   0.028s] ( 117/5472) aion-awl mir::deferred_tests::sort_non_comparable_key_refuses_with_the_reference_class
        PASS [   0.029s] ( 118/5472) aion-awl mir::deferred_tests::ordinary_call_index_lowers_with_the_anchored_runtime_message
        PASS [   0.032s] ( 119/5472) aion-awl mir::deferred_tests::statement_level_child_forms_plan_the_witness
        PASS [   0.030s] ( 120/5472) aion-awl mir::deferred_tests::value_route_payloads_now_lower
        PASS [   0.006s] ( 121/5472) aion-awl mir::harness_control_tests::the_control_pair_differs_only_by_the_section
        PASS [   0.007s] ( 122/5472) aion-awl mir::harness_control_tests::a_harness_section_does_not_change_the_lowering
        PASS [   0.033s] ( 123/5472) aion-awl mir::deferred_tests::substep_fixtures_now_lower
        PASS [   0.038s] ( 124/5472) aion-awl mir::deferred_tests::subflow_declarations_now_lower
        PASS [   0.005s] ( 125/5472) aion-awl mir::runtime::tests::enumeration_covers_every_variant_exactly_once
        PASS [   0.027s] ( 126/5472) aion-awl mir::fork_tests::named_child_branch_refuses_cleanly
        PASS [   0.005s] ( 127/5472) aion-awl mir::runtime::tests::query_handler_is_not_a_durable_effect
        PASS [   0.032s] ( 128/5472) aion-awl mir::fork_tests::fork_branch_call_site_config_lowers_with_per_key_merge
        PASS [   0.006s] ( 129/5472) aion-awl mir::runtime::tests::import_surface_projects_the_full_enumeration
        PASS [   0.036s] ( 130/5472) aion-awl mir::fork_tests::action_and_child_collection_forks_route_distinctly
        PASS [   0.032s] ( 131/5472) aion-awl mir::fork_tests::homogeneous_named_fork_pins_one_typed_all
        PASS [   0.034s] ( 132/5472) aion-awl mir::fork_tests::heterogeneous_named_fork_pins_raw_twins_and_positional_decode
        PASS [   0.006s] ( 133/5472) aion-awl mir::select::control_tests::assert_list_badmatch_traps_the_subject_list
        PASS [   0.056s] ( 134/5472) aion-awl mir::deferred_tests::parallel_layer_fixtures_now_lower
        PASS [   0.028s] ( 135/5472) aion-awl mir::fork_tests::sequential_fork_pins_fold_initial_state_and_result_order
        PASS [   0.030s] ( 136/5472) aion-awl mir::fork_tests::parallel_action_fork_pins_map_fanout_semantics
        PASS [   0.007s] ( 137/5472) aion-awl mir::select::inspect_tests::framed_return_reloads_the_return_value_into_x0
        PASS [   0.028s] ( 138/5472) aion-awl mir::pipe_tests::pipe_child_stage_lowers_to_spawn_and_wait
        PASS [   0.006s] ( 139/5472) aion-awl mir::select::inspect_tests::marshaling_rejects_swapped_args_dropped_store_and_wrong_arity
        PASS [   0.005s] ( 140/5472) aion-awl mir::select::inspect_tests::return_keeps_x0_live_across_a_preceding_heap_op
        PASS [   0.036s] ( 141/5472) aion-awl mir::fork_tests::stopgap_refusals_match_the_reference_classes
        PASS [   0.028s] ( 142/5472) aion-awl mir::pipe_tests::sort_string_key_selects_string_compare
        PASS [   0.029s] ( 143/5472) aion-awl mir::pipe_tests::post_stage_any_all_lower_through_predicate_closures
        PASS [   0.006s] ( 144/5472) aion-awl mir::select::inspect_tests::targeted_framed_function_brackets_and_single_exits
        PASS [   0.032s] ( 145/5472) aion-awl mir::pipe_tests::combinator_stages_lower_to_list_runtime_calls
        PASS [   0.007s] ( 146/5472) aion-awl mir::select::inspect_tests::targeted_frameless_body_uses_no_frame_or_y
        PASS [   0.007s] ( 147/5472) aion-awl mir::select::inspect_tests::targeted_heap_ops_declare_accurate_live
        PASS [   0.032s] ( 148/5472) aion-awl mir::select::control_tests::enum_total_tail_selects_with_explicit_case_end_trap
        PASS [   0.032s] ( 149/5472) aion-awl mir::select::control_tests::short_circuit_optional_selects_checked_assert_some
        PASS [   0.036s] ( 150/5472) aion-awl mir::select::control_tests::outcome_guard_decision_tree_uses_real_failure_labels
        PASS [   0.037s] ( 151/5472) aion-awl mir::select::control_tests::loop_counter_selects_a_gc_bif2_against_erlang_plus
        PASS [   0.035s] ( 152/5472) aion-awl mir::select::inspect_tests::awl_hello_outcome_route_returns_not_tail_calls
        PASS [   0.006s] ( 153/5472) aion-awl mir::select::tests::shape_call_import
        PASS [   0.006s] ( 154/5472) aion-awl mir::select::tests::shape_call_local
        PASS [   0.006s] ( 155/5472) aion-awl mir::select::tests::shape_field_get
        PASS [   0.006s] ( 156/5472) aion-awl mir::select::tests::shape_frameless_tail_import
        PASS [   0.005s] ( 157/5472) aion-awl mir::select::tests::shape_record_tuple
        PASS [   0.006s] ( 158/5472) aion-awl mir::select::tests::shape_json_obj_two_pairs
        PASS [   0.006s] ( 159/5472) aion-awl mir::select::tests::shape_make_closure
        PASS [   0.006s] ( 160/5472) aion-awl mir::select::tests::shape_record_with_literal_int
        PASS [   0.031s] ( 161/5472) aion-awl mir::select::tests::dead_body_is_a_real_sidecar_visible_function
        PASS [   0.006s] ( 162/5472) aion-awl mir::select::tests::shape_record_zero_field
        PASS [   0.006s] ( 163/5472) aion-awl mir::select::tests::shape_try_bind
        PASS [   0.034s] ( 164/5472) aion-awl mir::select::inspect_tests::targeted_loop_recursion_is_a_self_tail_call
        PASS [   0.033s] ( 165/5472) aion-awl mir::select::tests::minimal_module_has_the_expected_abi_structure
        PASS [   0.030s] ( 166/5472) aion-awl mir::select::tests::run_shell_passes_raw_payload_not_the_closure
        PASS [   0.030s] ( 167/5472) aion-awl mir::select::tests::selection_is_deterministic
        PASS [   0.041s] ( 168/5472) aion-awl mir::select::inspect_tests::targeted_route_to_step_is_a_local_tail_call
        PASS [   0.006s] ( 169/5472) aion-awl positions::tests::a_multibyte_span_covers_its_full_extent_in_utf16_units
        PASS [   0.005s] ( 170/5472) aion-awl positions::tests::an_inverted_span_collapses_to_its_start
        PASS [   0.005s] ( 171/5472) aion-awl positions::tests::offsets_past_the_document_clamp_to_its_end
        PASS [   0.005s] ( 172/5472) aion-awl positions::tests::positions_are_utf16_not_bytes_or_scalar_columns
        PASS [   0.005s] ( 173/5472) aion-awl positions::tests::point_spans_stay_points
        PASS [   0.005s] ( 174/5472) aion-awl printer::harness::tests::the_scrambled_fixture_really_is_scrambled
        PASS [   0.031s] ( 175/5472) aion-awl mir::tests::backward_route_reentry_now_lowers
        PASS [   0.005s] ( 176/5472) aion-awl printer::harness::tests::the_printer_emits_every_field_in_exactly_the_canonical_order
        PASS [   0.026s] ( 177/5472) aion-awl mir::tests::optional_rhs_is_nested_below_short_circuit_branch
        PASS [   0.167s] ( 178/5472) aion-awl guide::lookup::guide_tests::every_entry_example_passes_the_real_checker
        PASS [   0.027s] ( 179/5472) aion-awl mir::tests::minimal_exercises_core_op_shapes
        PASS [   0.031s] ( 180/5472) aion-awl mir::tests::counterless_loop_pins_the_scalar_result_path
        PASS [   0.005s] ( 181/5472) aion-awl workspace_root::tests::an_absolute_path_is_the_first_accepted_form
        PASS [   0.005s] ( 182/5472) aion-awl workspace_root::tests::a_leading_placeholder_is_the_second_accepted_form
        PASS [   0.005s] ( 183/5472) aion-awl workspace_root::tests::a_relative_path_is_refused_as_it_always_was
        PASS [   0.037s] ( 184/5472) aion-awl mir::tests::a_worker_document_is_refused_by_the_lowering
        PASS [   0.032s] ( 185/5472) aion-awl mir::tests::loop_lowering_pins_the_reference_semantics
        PASS [   0.034s] ( 186/5472) aion-awl mir::tests::compound_until_clones_control_only_into_unresolved_leaves
        PASS [   0.007s] ( 187/5472) aion-awl workspace_root::tests::the_new_form_did_not_widen_the_old_refusal
        PASS [   0.007s] ( 188/5472) aion-awl workspace_root::tests::an_empty_value_is_refused_before_either_form_is_considered
        PASS [   0.007s] ( 189/5472) aion-awl::action_bodies a_body_round_trips_and_prints_idempotently
        PASS [   0.033s] ( 190/5472) aion-awl mir::tests::typed_codec_bodies_pin_the_wire_shapes
        PASS [   0.005s] ( 191/5472) aion-awl::action_bodies a_declared_body_parses_onto_its_action_and_a_bodyless_action_keeps_none
        PASS [   0.006s] ( 192/5472) aion-awl::action_bodies a_body_written_before_the_config_line_normalizes_to_canonical_order
        PASS [   0.005s] ( 193/5472) aion-awl::action_bodies comments_around_a_body_survive_the_round_trip
        PASS [   0.005s] ( 194/5472) aion-awl::action_body_executor_parity a_parameter_in_program_position_is_refused_by_both
        PASS [   0.006s] ( 195/5472) aion-awl::action_bodies two_config_lines_on_one_action_are_a_parse_error
        PASS [   0.005s] ( 196/5472) aion-awl::action_body_executor_parity the_accepted_table_is_actually_accepted
        PASS [   0.006s] ( 197/5472) aion-awl::action_body_executor_parity every_command_the_checker_accepts_the_executor_also_accepts
        PASS [   0.008s] ( 198/5472) aion-awl::action_bodies two_bodies_on_one_action_are_a_parse_error
        PASS [   0.005s] ( 199/5472) aion-awl::action_body_executor_parity the_refused_table_is_actually_refused
        PASS [   0.007s] ( 200/5472) aion-awl::assistant_example nudge_cap_exhaustion_always_ends_the_session
        PASS [   0.007s] ( 201/5472) aion-awl::assistant_example the_round_loop_dispatches_exactly_once_per_iteration
        PASS [   0.031s] ( 202/5472) aion-awl::action_bodies a_bodyless_document_still_checks_clean
        PASS [   0.030s] ( 203/5472) aion-awl::action_bodies run_is_still_usable_as_an_ordinary_name
        PASS [   0.008s] ( 204/5472) aion-awl::assistant_example the_wait_loop_dispatches_nothing
        PASS [   0.033s] ( 205/5472) aion-awl::action_bodies escapes_inside_a_command_survive_the_round_trip
        PASS [   0.035s] ( 206/5472) aion-awl::action_bodies a_document_with_no_bodies_compiles_to_no_implementations
        PASS [   0.006s] ( 207/5472) aion-awl::awl2_defects action_config_duration_span_matches_flagship_true_position
        PASS [   0.005s] ( 208/5472) aion-awl::awl2_defects dead_header_keywords_get_targeted_fix_its
        PASS [   0.006s] ( 209/5472) aion-awl::awl2_defects comment_torture_document_round_trips_byte_identically
        PASS [   0.004s] ( 210/5472) aion-awl::awl2_defects error_column_after_multibyte_prose_is_character_correct
        PASS [   0.007s] ( 211/5472) aion-awl::awl2_defects dead_statement_keywords_get_targeted_fix_its
        PASS [   0.005s] ( 212/5472) aion-awl::awl2_defects fork_collection_span_matches_flagship_true_position
        PASS [   0.037s] ( 213/5472) aion-awl::action_bodies the_compiled_workflow_carries_the_command_and_its_binding
        PASS [   0.005s] ( 214/5472) aion-awl::awl2_defects guard_error_at_exactly_line_forty_is_source_correct
        PASS [   0.005s] ( 215/5472) aion-awl::awl2_defects guard_error_deep_in_document_reports_true_line_and_offset
        PASS [   0.033s] ( 216/5472) aion-awl::assistant_example the_assistant_example_checks_clean_and_is_canonically_formatted
        PASS [   0.005s] ( 217/5472) aion-awl::awl2_defects inline_schema_error_column_after_multibyte_content_is_character_correct
        PASS [   0.039s] ( 218/5472) aion-awl::action_body_executor_parity the_compiled_binding_matches_what_the_executor_would_parse
        PASS [   0.006s] ( 219/5472) aion-awl::b2_review_regressions a_self_routing_substep_without_a_bound_is_rejected
        PASS [   0.008s] ( 220/5472) aion-awl::awl2_defects own_line_comments_survive_at_every_nesting_level
        PASS [   0.007s] ( 221/5472) aion-awl::b2_review_regressions a_failure_route_does_not_inherit_success_bindings
        PASS [   0.006s] ( 222/5472) aion-awl::b2_review_regressions a_failure_route_into_a_collect_breaks_definite_assignment
        PASS [   0.006s] ( 223/5472) aion-awl::b2_review_regressions a_sub_cycle_avoiding_the_bounded_vertex_is_rejected
        PASS [   0.005s] ( 224/5472) aion-awl::b2_review_regressions a_substep_visits_bound_is_type_checked
        PASS [   0.006s] ( 225/5472) aion-awl::b2_review_regressions a_subflow_parameter_cannot_shadow_a_const
        PASS [   0.038s] ( 226/5472) aion-awl::assistant_example the_emitted_round_loop_exits_on_the_stalled_field
        PASS [   0.007s] ( 227/5472) aion-awl::b2_review_regressions incompatible_per_path_collected_types_are_rejected
        PASS [   0.007s] ( 228/5472) aion-awl::b2_review_regressions an_unbounded_sibling_substep_cycle_is_rejected
        PASS [   0.006s] ( 229/5472) aion-awl::b2_review_regressions the_failure_binding_document_is_refused_before_emission
        PASS [   0.007s] ( 230/5472) aion-awl::b2_review_round2 a_pure_sibling_after_cycle_is_rejected
        PASS [   0.007s] ( 231/5472) aion-awl::b2_review_round2 a_sibling_substep_after_cycle_is_rejected
        PASS [   0.006s] ( 232/5472) aion-awl::b2_review_round2 an_unknown_sibling_after_target_is_rejected
        PASS [   0.007s] ( 233/5472) aion-awl::b2_review_round2 a_structurally_equal_join_rejects_incompatible_consumption
        PASS [   0.051s] ( 234/5472) aion-awl::assistant_example the_assistant_example_compiles
        PASS [   0.007s] ( 235/5472) aion-awl::b2_review_round2 incompatible_types_meeting_at_a_join_are_rejected_there
        PASS [   0.032s] ( 236/5472) aion-awl::b2_review_regressions a_bounded_sibling_substep_cycle_is_accepted
        PASS [   0.009s] ( 237/5472) aion-awl::builtin_reservation every_reserved_name_is_refused_by_both_surfaces_for_the_same_reason
        PASS [   0.027s] ( 238/5472) aion-awl::b2_review_regressions bounds_covering_every_overlapping_cycle_check_clean
        PASS [   0.030s] ( 239/5472) aion-awl::b2_review_regressions compensation_bindings_before_the_failure_route_do_flow
        PASS [   0.031s] ( 240/5472) aion-awl::b2_review_regressions same_named_subflow_outcomes_resolve_to_their_own_declarations
        PASS [   0.033s] ( 241/5472) aion-awl::b2_review_round2 a_structurally_equal_join_keeps_a_concrete_type
        PASS [   0.078s] ( 242/5472) aion-awl::assistant_example the_assistant_example_answers_the_status_query
        PASS [   0.031s] ( 243/5472) aion-awl::b2_review_round2 disjoint_branches_may_reuse_a_name_at_different_types
        PASS [   0.029s] ( 244/5472) aion-awl::b2_review_round2 substep_after_is_refused_at_emission
        PASS [   0.006s] ( 245/5472) aion-awl::checker check_without_a_root_reports_unresolvable_imports
        PASS [   0.034s] ( 246/5472) aion-awl::builtin_reservation checker_and_emitter_acceptance_round_trip_for_reserved_names_and_control
        PASS [   0.007s] ( 247/5472) aion-awl::checker diagnostics_arrive_sorted_by_source_position
        PASS [   0.007s] ( 248/5472) aion-awl::checker duplicate_step_names_are_refused
        PASS [   0.030s] ( 249/5472) aion-awl::builtin_reservation ordinary_name_passes_both_surfaces
        PASS [   0.029s] ( 250/5472) aion-awl::canonical_emission authored_key_order_cannot_change_the_emitted_bytes
        PASS [   0.006s] ( 251/5472) aion-awl::checker imported_optional_field_is_optional_in_the_projection_direction
        PASS [   0.032s] ( 252/5472) aion-awl::builtin_reservation schema_of_keeps_referenceable_and_reserved_roles_distinct
        PASS [   0.029s] ( 253/5472) aion-awl::canonical_emission derived_schemas_serialize_key_sorted
        PASS [   0.007s] ( 254/5472) aion-awl::checker unknown_binding_error_is_span_exact
        PASS [   0.006s] ( 255/5472) aion-awl::checker_hardening inline_schema_error_anchors_at_the_offending_nested_token
        PASS [   0.007s] ( 256/5472) aion-awl::checker_hardening call_site_config_on_a_child_call_is_refused
        PASS [   0.034s] ( 257/5472) aion-awl::canonical_emission folded_schema_text_is_key_sorted_in_the_emitted_source
        PASS [   0.009s] ( 258/5472) aion-awl::checker_hardening deep_record_mismatch_is_refused_past_the_old_depth_cap
        PASS [   0.007s] ( 259/5472) aion-awl::checker_hardening inline_schema_refusal_skips_an_earlier_property_named_like_the_keyword
        PASS [   0.031s] ( 260/5472) aion-awl::canonical_emission the_sidecar_matches_a_key_sorted_expectation
        PASS [   0.034s] ( 261/5472) aion-awl::canonical_emission projected_record_fields_are_emitted_in_key_order
        PASS [   0.007s] ( 262/5472) aion-awl::checker_hardening statements_after_an_unconditional_route_are_refused
        PASS [   0.059s] ( 263/5472) aion-awl::builtin_reservation emitter_preclaims_every_name_the_checker_reserves
        PASS [   0.009s] ( 264/5472) aion-awl::checker_hardening outcome_clauses_behind_a_body_terminal_route_are_refused
        PASS [   0.008s] ( 265/5472) aion-awl::checker_regressions backward_route_re_armed_by_an_after_edge_needs_a_bound
        PASS [   0.006s] ( 266/5472) aion-awl::checker_regressions dangling_middle_step_is_refused
        PASS [   0.007s] ( 267/5472) aion-awl::checker_regressions loop_max_may_not_read_loop_locals
        PASS [   0.007s] ( 268/5472) aion-awl::checker_regressions loop_rebind_inside_a_collection_fork_branch_does_not_count
        PASS [   0.007s] ( 269/5472) aion-awl::checker_regressions named_fork_branch_cannot_read_a_sibling_bindings
        PASS [   0.010s] ( 270/5472) aion-awl::checker_regressions loop_max_may_not_read_the_counter
        PASS [   0.007s] ( 271/5472) aion-awl::checker_regressions named_fork_join_binding_is_refused
        PASS [   0.030s] ( 272/5472) aion-awl::checker route_targeted_earlier_step_reads_a_later_steps_binding
        PASS [   0.008s] ( 273/5472) aion-awl::checker_regressions piped_route_to_a_step_is_refused
        PASS [   0.007s] ( 274/5472) aion-awl::checker_regressions step_sharing_a_workflow_outcome_name_is_refused
        PASS [   0.006s] ( 275/5472) aion-awl::checker_rulings list_element_optional_is_refused_in_input_position
        PASS [   0.008s] ( 276/5472) aion-awl::checker_rulings list_element_optional_is_refused_in_outcome_and_child_positions
        PASS [   0.006s] ( 277/5472) aion-awl::checker_rulings loop_inside_a_substep_answers_to_the_substep
        PASS [   0.009s] ( 278/5472) aion-awl::checker_rulings loop_inside_a_fork_branch_still_requires_outcome_clauses
        PASS [   0.006s] ( 279/5472) aion-awl::checker_rulings route_nested_in_a_fork_inside_a_loop_is_refused_by_check
        PASS [   0.007s] ( 280/5472) aion-awl::checker_rulings nested_list_element_optional_is_refused
        PASS [   0.032s] ( 281/5472) aion-awl::checker_hardening structurally_equal_recursive_types_are_compatible_and_terminate
        PASS [   0.034s] ( 282/5472) aion-awl::checker_regressions falling_step_consumed_by_an_after_dependent_is_legal
        PASS [   0.033s] ( 283/5472) aion-awl::checker_regressions loop_max_over_inputs_stays_clean
        PASS [   0.027s] ( 284/5472) aion-awl::checker_rulings optional_list_stays_legal
        PASS [   0.028s] ( 285/5472) aion-awl::command_emit a_declaration_with_no_timeout_emits_no_timeout
        PASS [   0.029s] ( 286/5472) aion-awl::checker_rulings routes_outside_loop_bodies_stay_legal
        PASS [   0.027s] ( 287/5472) aion-awl::command_emit a_declared_timeout_emits_its_own_number_and_owner
        PASS [   0.029s] ( 288/5472) aion-awl::command_emit a_declared_default_fills_an_omitted_parameter
        PASS [   0.075s] ( 289/5472) aion-awl::checker every_check_staged_invalid_fixture_is_rejected_as_expected
        PASS [   0.030s] ( 290/5472) aion-awl::command_emit a_list_operand_emits_one_element_per_item
        PASS [   0.028s] ( 291/5472) aion-awl::command_emit a_refusing_invocation_never_carries_the_marker
        PASS [   0.029s] ( 292/5472) aion-awl::command_emit a_refusing_invocation_refuses_a_leading_dash_operand
        PASS [   0.007s] ( 293/5472) aion-awl::command_gate_census every_fixture_cites_a_specimen_that_is_still_at_its_pinned_digest
        PASS [   0.028s] ( 294/5472) aion-awl::command_emit a_value_containing_a_space_stays_one_argv_element
        PASS [   0.006s] ( 295/5472) aion-awl::command_gate_census the_normalizer_rejects_a_command_line_that_is_not_the_census
        PASS [   0.006s] ( 296/5472) aion-awl::command_just_subset every_absorbed_production_and_every_refused_form_has_a_fixture
        PASS [   0.025s] ( 297/5472) aion-awl::command_emit an_admitting_invocation_carries_the_marker_before_its_operands
        PASS [   0.026s] ( 298/5472) aion-awl::command_emit an_admitting_invocation_with_no_operands_emits_no_marker
        PASS [   0.005s] ( 299/5472) aion-awl::command_just_subset the_fixture_corpus_is_discovered_and_not_empty
        PASS [   0.006s] ( 300/5472) aion-awl::command_just_subset every_fixture_records_its_provenance
        PASS [   0.026s] ( 301/5472) aion-awl::command_emit the_parameter_set_refusals_each_fire_with_their_own_cause
        PASS [   0.030s] ( 302/5472) aion-awl::command_emit env_cwd_and_hardening_resolve_structurally
        PASS [   0.028s] ( 303/5472) aion-awl::command_emit firing_a_declared_specimen_proves_the_refusal_is_load_bearing
        PASS [   0.005s] ( 304/5472) aion-awl::command_refusals every_cause_has_a_refusing_specimen
        PASS [   0.029s] ( 305/5472) aion-awl::command_emit the_marker_is_placed_whatever_the_operands_turn_out_to_be
        PASS [   0.008s] ( 306/5472) aion-awl::command_just_subset the_refused_table_and_the_refusal_fixtures_name_the_same_causes
        PASS [   0.005s] ( 307/5472) aion-awl::command_specimen a_specimen_that_is_accepted_fails_at_the_specimen_layer
        PASS [   0.005s] ( 308/5472) aion-awl::command_specimen a_specimen_swallowed_by_its_fill_fails_at_the_specimen_layer
        PASS [   0.005s] ( 309/5472) aion-awl::command_specimen an_admitting_invocation_has_no_specimen_to_fire
        PASS [   0.008s] ( 310/5472) aion-awl::command_refusals every_specimen_refuses_at_the_layer_its_cause_names
        PASS [   0.006s] ( 311/5472) aion-awl::command_specimen the_canonical_refusing_fixtures_specimen_fires_and_refuses
        PASS [   0.014s] ( 312/5472) aion-awl::command_refusals every_document_specimen_refuses_with_its_own_cause_and_bytes
        PASS [   0.037s] ( 313/5472) aion-awl::command_emit the_same_declaration_and_parameters_emit_byte_identical_argv
        PASS [   0.006s] ( 314/5472) aion-awl::command_specimen the_wrong_cause_code_is_filed_at_the_specimen_layer_though_its_red_arm_is_shadowed
        PASS [   0.033s] ( 315/5472) aion-awl::command_gate_census one_shape_and_four_parameter_sets_emit_the_four_rigs_census_command_lines
        PASS [   0.007s] ( 316/5472) aion-awl::compile language_errors_pass_through_verbatim
        PASS [   0.005s] ( 317/5472) aion-awl::compile lowering_display_parity_for_message_and_planning
        PASS [   0.035s] ( 318/5472) aion-awl::command_just_subset every_fixture_meets_its_declared_expectation
        PASS [   0.006s] ( 319/5472) aion-awl::compile multi_outcome_envelope_derives_one_arm_per_outcome
        PASS [   0.031s] ( 320/5472) aion-awl::command_refusals the_two_canonical_shapes_are_accepted
        PASS [   0.027s] ( 321/5472) aion-awl::compile declared_node_and_override_both_surface
        PASS [   0.033s] ( 322/5472) aion-awl::compile awl_hello_contracts_match_the_reference_shapes
        PASS [   0.036s] ( 323/5472) aion-awl::compile awl_hello_actions_are_the_two_unpinned_requirements
        PASS [   0.034s] ( 324/5472) aion-awl::compile call_site_node_override_yields_the_override
        PASS [   0.008s] ( 325/5472) aion-awl::compile_visits_scope visits_route_payload_is_refused_by_the_shared_check_before_lowering
        PASS [   0.007s] ( 326/5472) aion-awl::concat_lowering checked_compile_rejects_mixed_concat_before_lowering
        PASS [   0.006s] ( 327/5472) aion-awl::concat_lowering direct_lower_refuses_concat_unless_both_operands_are_strings
        PASS [   0.029s] ( 328/5472) aion-awl::compile mutually_recursive_payload_falls_back_via_trial_defs
        PASS [   0.029s] ( 329/5472) aion-awl::compile recursive_payload_derives_through_defs_not_inline
        PASS [   0.006s] ( 330/5472) aion-awl::corpus_roundtrip the_comparison_rejects_an_outcome_the_tree_does_not_produce
        PASS [   0.009s] ( 331/5472) aion-awl::concat_lowering int_alias_plus_string_still_refuses_at_direct_lowering
        PASS [   0.005s] ( 332/5472) aion-awl::corpus_roundtrip the_walk_excludes_exactly_the_three_named_classes
        PASS [   0.052s] ( 333/5472) aion-awl::compile awl_hello_compiles_deterministically_to_the_select_bytes
        PASS [   0.034s] ( 334/5472) aion-awl::compile refusal_passthrough_is_lossless
        PASS [   0.005s] ( 335/5472) aion-awl::editor_assets every_editor_covers_every_node_the_authoritative_query_colours
        PASS [   0.051s] ( 336/5472) aion-awl::compile fork_action_fanout_compiles_deterministically_to_the_select_bytes
        PASS [   0.005s] ( 337/5472) aion-awl::editor_assets grammar_builtin_types_match_the_awl_authority
        PASS [   0.004s] ( 338/5472) aion-awl::editor_assets the_recorded_pin_is_present_on_the_public_mirror
        PASS [   0.012s] ( 339/5472) aion-awl::editor_assets no_query_names_a_node_the_grammar_does_not_produce
        PASS [   0.337s] ( 340/5472) aion-awl mir::select::inspect_tests::wrong_live_expectation_goes_red
        PASS [   0.034s] ( 341/5472) aion-awl::concat_lowering checker_approved_string_alias_concat_lowers_and_selects
        PASS [   0.019s] ( 342/5472) aion-awl::editor_assets the_zed_pin_names_a_mirror_commit_carrying_this_grammar
        PASS [   0.032s] ( 343/5472) aion-awl::declared_signal_contract every_declared_signal_reaches_the_contract
        PASS [   0.033s] ( 344/5472) aion-awl::declared_signal_contract each_signal_carries_its_own_declared_schema
        PASS [   0.035s] ( 345/5472) aion-awl::declared_signal_contract the_declared_schema_admits_conforming_and_refuses_malformed
        PASS [   0.062s] ( 346/5472) aion-awl::compile_child_fork child_collection_fork_compiles_deterministically_to_the_select_bytes
        PASS [   0.029s] ( 347/5472) aion-awl::emitter child_tests::child_call_lowers_to_string_name_spawn
        PASS [   0.005s] ( 348/5472) aion-awl::emitter compile_tests::the_compile_proof_builds_where_cargo_clean_can_reach_it
        PASS [   0.027s] ( 349/5472) aion-awl::emitter child_tests::named_fork_child_refusal_is_honest
        PASS [   0.038s] ( 350/5472) aion-awl::emitter child_tests::child_collection_fork_spawns_all_then_awaits_all
        PASS [   0.030s] ( 351/5472) aion-awl::emitter child_tests::spawn_lowers_detached
        PASS [   0.031s] ( 352/5472) aion-awl::emitter codec_bodies_pin_the_same_wire_fragments
        PASS [   0.384s] ( 353/5472) aion-awl mir::select::inspect_tests::make_fun_capture_reads_are_modeled_and_a_removed_reload_is_caught
        PASS [   0.032s] ( 354/5472) aion-awl::emitter conflicting_binding_types_across_branches_are_refused
        PASS [   0.029s] ( 355/5472) aion-awl::emitter enum_total_outcomes_lower_to_case
        PASS [   0.027s] ( 356/5472) aion-awl::emitter float_orderings_render_float_operators
        PASS [   0.034s] ( 357/5472) aion-awl::emitter flagship_scout_and_warm_build_share_one_parallel_all
        PASS [   0.046s] ( 358/5472) aion-awl::emitter emitted_sources_avoid_nondeterministic_apis
        PASS [   0.033s] ( 359/5472) aion-awl::emitter fork_forms_map_and_fold
        PASS [   0.005s] ( 360/5472) aion-awl::emitter gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.004s] ( 361/5472) aion-awl::emitter gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.005s] ( 362/5472) aion-awl::emitter gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.028s] ( 363/5472) aion-awl::emitter fork_tests::homogeneous_named_fork_is_one_typed_all
        PASS [   0.005s] ( 364/5472) aion-awl::emitter gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 365/5472) aion-awl::emitter gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.258s] ( 366/5472) aion-awl::checker every_valid_fixture_checks_clean
        PASS [   0.007s] ( 367/5472) aion-awl::emitter gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.028s] ( 368/5472) aion-awl::emitter fork_tests::sequential_fork_folds_from_empty_and_reverses
        PASS [   0.024s] ( 369/5472) aion-awl::emitter gleam_keywords_are_sanitized_everywhere
        PASS [   0.035s] ( 370/5472) aion-awl::emitter fork_tests::parallel_action_fork_is_one_map_with_error_collapse
        PASS [   0.007s] ( 371/5472) aion-awl::emitter loop_tests::counting_name_collision_is_checker_refused
        PASS [   0.006s] ( 372/5472) aion-awl::emitter loop_tests::route_inside_a_loop_body_is_refused
        PASS [   0.006s] ( 373/5472) aion-awl::emitter loop_tests::loop_max_must_be_loop_invariant
        PASS [   0.006s] ( 374/5472) aion-awl::emitter loop_tests::unchecked_counter_is_not_in_until_scope
        PASS [   0.005s] ( 375/5472) aion-awl::emitter piped_route_with_payload_is_refused_at_emit
        PASS [   0.026s] ( 376/5472) aion-awl::emitter loop_tests::compound_until_is_short_circuited_in_nested_loops
        PASS [   0.026s] ( 377/5472) aion-awl::emitter loop_counter_is_language_owned
        PASS [   0.026s] ( 378/5472) aion-awl::emitter loop_tests::counterless_loop_uses_the_scalar_result_path
        PASS [   0.031s] ( 379/5472) aion-awl::emitter heterogeneous_named_fork_dispatches_one_parallel_all
        PASS [   0.009s] ( 380/5472) aion-awl::emitter schema_import_without_root_is_a_spanned_emit_error
        PASS [   0.006s] ( 381/5472) aion-awl::fallback_config empty_and_soft_keyword_entries_are_parse_errors
        PASS [   0.007s] ( 382/5472) aion-awl::fallback_config duplicate_fallback_queue_is_anchored_on_the_second_occurrence
        PASS [   0.005s] ( 383/5472) aion-awl::fallback_config fallback_is_declaration_only
        PASS [   0.027s] ( 384/5472) aion-awl::emitter on_failure_compensates_then_routes
        PASS [   0.029s] ( 385/5472) aion-awl::emitter nested_boolean_expressions_use_gleam_block_grouping
        PASS [   0.005s] ( 386/5472) aion-awl::fallback_config fallback_then_retry_is_two_config_keys
        PASS [   0.025s] ( 387/5472) aion-awl::emitter wait_timeout_binds_optionally
        PASS [   0.005s] ( 388/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.033s] ( 389/5472) aion-awl::emitter sequential_layer_fallback_is_announced
        PASS [   0.005s] ( 390/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.005s] ( 391/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.031s] ( 392/5472) aion-awl::fallback_config fallback_round_trips_and_lowers_after_retry
        PASS [   0.005s] ( 393/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 394/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.029s] ( 395/5472) aion-awl::feature_any_all_workflow_id workflow_id_has_reference_direct_structural_parity
        PASS [   0.035s] ( 396/5472) aion-awl::feature_any_all_workflow_id fallible_predicates_have_result_callbacks_and_outer_binds
        PASS [   0.005s] ( 397/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.005s] ( 398/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.006s] ( 399/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.037s] ( 400/5472) aion-awl::feature_any_all_workflow_id nested_any_all_has_reference_direct_structural_parity
        PASS [   0.005s] ( 401/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.006s] ( 402/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.005s] ( 403/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.005s] ( 404/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.006s] ( 405/5472) aion-awl::flow_vocab_ergonomics an_expression_headed_statement_still_needs_a_terminator
        PASS [   0.006s] ( 406/5472) aion-awl::flow_vocab_ergonomics bindings_cannot_shadow_consts
        PASS [   0.006s] ( 407/5472) aion-awl::flow_vocab_ergonomics const_values_reject_runtime_reads
        PASS [   0.007s] ( 408/5472) aion-awl::flow_vocab_ergonomics const_cycles_are_rejected_at_the_backward_reference
        PASS [   0.006s] ( 409/5472) aion-awl::flow_vocab_ergonomics duplicate_consts_are_rejected_at_the_second_declaration
        PASS [   0.007s] ( 410/5472) aion-awl::flow_vocab_ergonomics emit_refuses_documents_that_do_not_check_cleanly
        PASS [   0.007s] ( 411/5472) aion-awl::flow_vocab_ergonomics invalid_json_body_points_into_the_body
        PASS [   0.005s] ( 412/5472) aion-awl::flow_vocab_ergonomics invalid_json_error_span_stays_on_char_boundaries_for_non_ascii
        PASS [   0.006s] ( 413/5472) aion-awl::flow_vocab_ergonomics json_is_a_reserved_word_at_declaration_sites
        PASS [   0.006s] ( 414/5472) aion-awl::flow_vocab_ergonomics malformed_multiline_json_string_is_reported_inside_the_body
        PASS [   0.005s] ( 415/5472) aion-awl::flow_vocab_ergonomics mir_lower_refuses_documents_that_do_not_check_cleanly
        PASS [   0.004s] ( 416/5472) aion-awl::flow_vocab_ergonomics of_is_a_reserved_word_at_declaration_sites
        PASS [   0.030s] ( 417/5472) aion-awl::flow_vocab_ergonomics a_statement_may_start_with_a_string_literal
        PASS [   0.024s] ( 418/5472) aion-awl::flow_vocab_ergonomics consts_fold_through_emission_as_plain_strings
        PASS [   0.220s] ( 419/5472) aion-awl::corpus_roundtrip every_pre_existing_document_reads_exactly_as_it_did_at_the_baseline
        PASS [   0.005s] ( 420/5472) aion-awl::flow_vocab_ergonomics raw_string_value_is_verbatim_with_no_escape_processing
        PASS [   0.006s] ( 421/5472) aion-awl::flow_vocab_ergonomics schema_of_an_undeclared_type_is_rejected
        PASS [   0.004s] ( 422/5472) aion-awl::flow_vocab_ergonomics unterminated_raw_string_is_refused_at_the_opener
        PASS [   0.006s] ( 423/5472) aion-awl::flow_vocab_ergonomics unterminated_json_body_is_refused_at_the_open_brace
        PASS [   0.007s] ( 424/5472) aion-awl::flow_vocab_ergonomics undefined_const_references_are_rejected
        PASS [   0.004s] ( 425/5472) aion-awl::flow_vocab_regions a_collect_with_no_open_region_is_rejected
        PASS [   0.028s] ( 426/5472) aion-awl::flow_vocab_ergonomics json_body_brace_counting_respects_braces_inside_strings
        PASS [   0.005s] ( 427/5472) aion-awl::flow_vocab_regions a_collect_closes_the_nearest_open_region
        PASS [   0.006s] ( 428/5472) aion-awl::flow_vocab_regions a_distribute_step_may_not_carry_outcomes
        PASS [   0.006s] ( 429/5472) aion-awl::flow_vocab_regions a_route_out_of_the_region_is_rejected
        PASS [   0.007s] ( 430/5472) aion-awl::flow_vocab_regions a_route_entering_the_region_mid_track_is_rejected
        PASS [   0.024s] ( 431/5472) aion-awl::flow_vocab_ergonomics the_vocabulary_document_round_trips_byte_identically
        PASS [   0.005s] ( 432/5472) aion-awl::flow_vocab_regions a_route_to_the_collect_from_outside_is_rejected
        PASS [   0.005s] ( 433/5472) aion-awl::flow_vocab_regions an_after_edge_may_not_cross_the_region_boundary
        PASS [   0.005s] ( 434/5472) aion-awl::flow_vocab_regions a_route_to_a_flow_outcome_from_inside_is_rejected
        PASS [   0.028s] ( 435/5472) aion-awl::flow_vocab_ergonomics the_semantic_index_declares_consts_with_their_folded_type
        PASS [   0.006s] ( 436/5472) aion-awl::flow_vocab_regions collect_must_open_its_step
        PASS [   0.006s] ( 437/5472) aion-awl::flow_vocab_regions an_unclosed_region_is_rejected_at_its_opener
        PASS [   0.007s] ( 438/5472) aion-awl::flow_vocab_regions distribute_must_be_its_steps_only_line
        PASS [   0.005s] ( 439/5472) aion-awl::flow_vocab_regions distribute_needs_a_list_collection
        PASS [   0.006s] ( 440/5472) aion-awl::flow_vocab_regions region_locals_fall_out_of_scope_at_the_collect
        PASS [   0.033s] ( 441/5472) aion-awl::flow_vocab_refusals every_new_construct_emits_with_its_generated_shape
        PASS [   0.006s] ( 442/5472) aion-awl::flow_vocab_regions region_statements_are_step_level_only
        PASS [   0.019s] ( 443/5472) aion-awl::flow_vocab_regions an_in_region_loopback_is_legal_and_stays_inside
        PASS [   0.007s] ( 444/5472) aion-awl::flow_vocab_regions the_collected_binding_must_be_assigned_on_every_success_path
        PASS [   0.005s] ( 445/5472) aion-awl::flow_vocab_regions the_collected_binding_must_be_produced_inside_the_region
        PASS [   0.028s] ( 446/5472) aion-awl::flow_vocab_regions an_empty_collection_is_legal
        PASS [   0.030s] ( 447/5472) aion-awl::flow_vocab_regions nested_regions_with_inner_loopback_check_clean
        PASS [   0.591s] ( 448/5472) aion-awl mir::select::tests::every_lowered_fixture_emits_and_validates
        PASS [   0.020s] ( 449/5472) aion-awl::flow_vocab_shape a_sequence_region_with_a_strict_collect_round_trips
        PASS [   0.023s] ( 450/5472) aion-awl::flow_vocab_shape collect_results_type_strict_and_tolerant_forms
        PASS [   0.024s] ( 451/5472) aion-awl::flow_vocab_shape every_step_carries_its_kind_in_the_semantic_index
        PASS [   0.004s] ( 452/5472) aion-awl::flow_vocab_subflows a_step_level_max_must_end_in_visits
        PASS [   0.023s] ( 453/5472) aion-awl::flow_vocab_shape every_step_carries_its_region_membership_in_the_semantic_index
        PASS [   0.019s] ( 454/5472) aion-awl::flow_vocab_shape the_shape_document_round_trips_byte_identically
        PASS [   0.023s] ( 455/5472) aion-awl::flow_vocab_shape sequence_steps_classify_as_sequence
        PASS [   0.005s] ( 456/5472) aion-awl::flow_vocab_subflows a_subflow_call_may_not_nest_inside_a_block
        PASS [   0.020s] ( 457/5472) aion-awl::flow_vocab_shape the_subflow_declaration_reaches_the_semantic_index
        PASS [   0.005s] ( 458/5472) aion-awl::flow_vocab_subflows a_subflow_call_must_be_its_steps_only_statement
        PASS [   0.005s] ( 459/5472) aion-awl::flow_vocab_subflows a_subflow_cannot_be_spawned
        PASS [   0.004s] ( 460/5472) aion-awl::flow_vocab_subflows a_subflow_declares_exactly_one_outcome
        PASS [   0.004s] ( 461/5472) aion-awl::flow_vocab_subflows a_subflow_is_not_a_pipe_stage
        PASS [   0.005s] ( 462/5472) aion-awl::flow_vocab_subflows a_subflow_captures_nothing_from_the_enclosing_flow
        PASS [   0.004s] ( 463/5472) aion-awl::flow_vocab_subflows a_subflow_outcome_carries_no_route
        PASS [   0.004s] ( 464/5472) aion-awl::flow_vocab_subflows a_subflow_without_an_outcome_is_refused
        PASS [   0.004s] ( 465/5472) aion-awl::flow_vocab_subflows collect_requires_its_result_binding
        PASS [   0.005s] ( 466/5472) aion-awl::flow_vocab_subflows a_value_route_payload_must_match_the_outcome_type
        PASS [   0.021s] ( 467/5472) aion-awl::flow_vocab_subflows a_subflow_call_binds_the_outcome_type
        PASS [   0.598s] ( 468/5472) aion-awl mir::tests::lowers_cover_and_golden
        PASS [   0.005s] ( 469/5472) aion-awl::flow_vocab_subflows subflow_recursion_is_rejected
        PASS [   0.005s] ( 470/5472) aion-awl::flow_vocab_visits a_cycle_bounded_only_by_an_inner_loop_is_now_rejected
        PASS [   0.005s] ( 471/5472) aion-awl::flow_vocab_visits a_cycle_rebinding_that_changes_the_type_is_rejected
        PASS [   0.005s] ( 472/5472) aion-awl::flow_vocab_visits a_non_cycle_step_still_may_not_rebind
        PASS [   0.005s] ( 473/5472) aion-awl::flow_vocab_visits the_visits_bound_must_be_an_int
        PASS [   0.004s] ( 474/5472) aion-awl::flow_vocab_visits the_visits_bound_must_be_input_derived
        PASS [   0.005s] ( 475/5472) aion-awl::flow_vocab_visits visits_is_unreadable_outside_the_bounded_steps_guards
        PASS [   0.006s] ( 476/5472) aion-awl::grammar_parser_gate every_assembly_form_is_refused_by_name_with_its_reason
        PASS [   0.027s] ( 477/5472) aion-awl::flow_vocab_subflows document_consts_stay_visible_inside_a_subflow
        PASS [   0.017s] ( 478/5472) aion-awl::flow_vocab_visits visits_in_a_bounded_guard_remains_clean_when_the_payload_uses_ordinary_data
        PASS [   0.024s] ( 479/5472) aion-awl::flow_vocab_visits a_cycle_step_may_rebind_a_cycle_threaded_name_keeping_its_type
        PASS [   0.022s] ( 480/5472) aion-awl::flow_vocab_visits a_visits_bound_on_a_cycle_member_makes_the_cycle_legal
        PASS [   0.007s] ( 481/5472) aion-awl::grammar_parser_gate the_top_level_offer_is_exactly_what_the_parser_admits
        PASS [   0.010s] ( 482/5472) aion-awl::grammar_parser_gate the_refusal_diagnostics_name_what_the_parser_actually_admits
        PASS [   0.010s] ( 483/5472) aion-awl::grammar_parser_gate worker::the_type_positions_offer_nothing_because_the_parser_admits_nothing
        PASS [   0.003s] ( 484/5472) aion-awl::guide_reference_gate embedded_reference_matches_the_authored_document
        PASS [   0.024s] ( 485/5472) aion-awl::grammar_parser_gate the_command_body_offer_is_openers_plus_witnessed_words
        PASS [   0.004s] ( 486/5472) aion-awl::guide_reference_gate every_cited_section_actually_discusses_the_word
        PASS [   0.005s] ( 487/5472) aion-awl::guide_reference_gate every_cited_section_exists_with_the_title_the_glossary_writes
        PASS [   0.004s] ( 488/5472) aion-awl::guide_reference_gate every_documented_path_the_glossary_names_exists
        PASS [   0.030s] ( 489/5472) aion-awl::grammar_parser_gate the_header_offer_is_openers_plus_witnessed_midline_words
        PASS [   0.004s] ( 490/5472) aion-awl::guide_reference_gate the_gate_rejects_a_citation_the_reference_cannot_answer
        PASS [   0.003s] ( 491/5472) aion-awl::guide_reference_gate the_whole_word_matcher_reads_boundaries
        PASS [   0.004s] ( 492/5472) aion-awl::guide_reference_gate the_inventory_covers_the_whole_vocabulary
        PASS [   0.003s] ( 493/5472) aion-awl::lexer dead_awl0_words_lex_as_plain_identifiers
        PASS [   0.003s] ( 494/5472) aion-awl::lexer dev_brief_loop_region_lexes_loop_counting_until_max
        PASS [   0.005s] ( 495/5472) aion-awl::lexer awl_hello_flagship_lexes_to_the_exact_token_sequence
        PASS [   0.026s] ( 496/5472) aion-awl::grammar_parser_gate worker::every_value_position_offer_is_witnessed_in_its_slot
        PASS [   0.004s] ( 497/5472) aion-awl::lexer doc_header_lines_lex_as_data_with_verbatim_text
        PASS [   0.003s] ( 498/5472) aion-awl::lexer doc_lines_lex_as_data_and_do_not_disturb_indentation
        PASS [   0.004s] ( 499/5472) aion-awl::lexer doc_line_spans_carry_true_position_and_marker
        PASS [   0.003s] ( 500/5472) aion-awl::lexer four_slashes_and_plain_comments_stay_trivia
        PASS [   0.004s] ( 501/5472) aion-awl::lexer keywords_embedded_in_longer_identifiers_stay_identifiers
        PASS [   0.005s] ( 502/5472) aion-awl::lexer every_rev2_keyword_lexes_as_a_keyword
        PASS [   0.028s] ( 503/5472) aion-awl::grammar_parser_gate worker::the_worker_block_offer_is_openers_plus_witnessed_words
        PASS [   0.003s] ( 504/5472) aion-awl::lexer trailing_comment_after_code_stays_trivia_before_newline
        PASS [   0.003s] ( 505/5472) aion-awl::lexer trailing_doc_header_marker_after_code_is_trivia_not_doc_data
        PASS [   0.004s] ( 506/5472) aion-awl::lexer trailing_triple_slash_after_code_is_trivia_not_doc_data
        PASS [   0.003s] ( 507/5472) aion-awl::lexer_spans bad_escape_reports_escape_span
        PASS [   0.003s] ( 508/5472) aion-awl::lexer_spans columns_after_multibyte_content_on_the_same_line_are_char_based
        PASS [   0.004s] ( 509/5472) aion-awl::lexer_spans crlf_newline_spans_cover_the_full_terminator
        PASS [   0.003s] ( 510/5472) aion-awl::lexer_spans lf_newline_span_is_the_single_terminator_byte
        PASS [   0.004s] ( 511/5472) aion-awl::lexer_spans lex_error_deep_in_document_reports_true_line_and_offset
        PASS [   0.003s] ( 512/5472) aion-awl::lexer_spans lone_dot_without_field_name_reports_span
        PASS [   0.024s] ( 513/5472) aion-awl::implicit_child_identity same_shape_parents_receive_distinct_reserved_child_types
        PASS [   0.003s] ( 514/5472) aion-awl::lexer_spans spans_stay_byte_correct_after_multibyte_doc_lines
        PASS [   0.003s] ( 515/5472) aion-awl::lexer_spans stray_character_reports_span
        PASS [   0.003s] ( 516/5472) aion-awl::lexer_spans over_indentation_jump_reports_a_lexical_error
        PASS [   0.004s] ( 517/5472) aion-awl::lexer_spans odd_indentation_reports_span
        PASS [   0.003s] ( 518/5472) aion-awl::lexer_spans tab_in_indentation_reports_span
        PASS [   0.003s] ( 519/5472) aion-awl::lexer_spans unterminated_string_reports_span
        PASS [   0.004s] ( 520/5472) aion-awl::lexer_spans token_deep_in_document_reports_true_line_and_offset
        PASS [   0.004s] ( 521/5472) aion-awl::lexer_spans tokens_expose_byte_line_and_column_spans
        PASS [   0.004s] ( 522/5472) aion-awl::operators_and_literals arrow_binds_call_results_and_accessors_chain
        PASS [   0.003s] ( 523/5472) aion-awl::operators_and_literals combinators_take_accessor_arguments
        PASS [   0.003s] ( 524/5472) aion-awl::operators_and_literals durations_lex_with_all_units_and_backoff_ranges
        PASS [   0.003s] ( 525/5472) aion-awl::operators_and_literals float_lexeme_is_preserved_verbatim
        PASS [   0.004s] ( 526/5472) aion-awl::operators_and_literals comparisons_booleans_and_string_concat_lex
        PASS [   0.003s] ( 527/5472) aion-awl::operators_and_literals enum_bar_is_distinct_from_pipe
        PASS [   0.003s] ( 528/5472) aion-awl::operators_and_literals inline_schema_json_tokens_lex
        PASS [   0.003s] ( 529/5472) aion-awl::operators_and_literals list_literals_and_literal_indexing_lex
        PASS [   0.004s] ( 530/5472) aion-awl::operators_and_literals fork_line_lexes_with_in_as_identifier
        PASS [   0.003s] ( 531/5472) aion-awl::operators_and_literals pipe_chain_with_accessor_stage_and_route_terminator
        PASS [   0.004s] ( 532/5472) aion-awl::operators_and_literals loop_seed_equal_and_schema_door_equal_lex_as_equal
        PASS [   0.031s] ( 533/5472) aion-awl::lexer entire_rev2_corpus_lexes_without_error
        PASS [   0.003s] ( 534/5472) aion-awl::operators_and_literals separated_unit_letter_is_not_a_duration
        PASS [   0.003s] ( 535/5472) aion-awl::operators_and_literals question_marks_optional_types_including_list_types
        PASS [   0.004s] ( 536/5472) aion-awl::operators_and_literals predicates_lex_as_keyword_pairs
        PASS [   0.003s] ( 537/5472) aion-awl::operators_and_literals string_escapes_booleans_and_floats_lex
        PASS [   0.004s] ( 538/5472) aion-awl::operators_and_literals spawn_join_wait_timeout_line_shapes_lex
        PASS [   0.004s] ( 539/5472) aion-awl::parser_corpus corpus_has_not_shrunk
        PASS [   0.004s] ( 540/5472) aion-awl::operators_and_literals unit_prefixed_identifier_is_not_a_duration
        PASS [   0.003s] ( 541/5472) aion-awl::parser_diagnostics call_headed_pipe_chain_gets_a_targeted_fix_it
        PASS [   0.004s] ( 542/5472) aion-awl::parser_diagnostics duplicate_loop_max_is_rejected_at_the_second_keyword
        PASS [   0.004s] ( 543/5472) aion-awl::parser_diagnostics duplicate_loop_until_is_rejected_at_the_second_keyword
        PASS [   0.004s] ( 544/5472) aion-awl::parser_diagnostics statement_after_an_outcome_clause_is_rejected
        PASS [   0.009s] ( 545/5472) aion-awl::parser_corpus every_parse_staged_invalid_fixture_fails_as_expected
        PASS [   0.005s] ( 546/5472) aion-awl::printer_roundtrip comma_tolerance_normalizes_to_canonical_form
        PASS [   0.005s] ( 547/5472) aion-awl::printer_roundtrip long_pipe_chains_break_before_each_stage
        PASS [   0.004s] ( 548/5472) aion-awl::printer_roundtrip stageless_bind_over_the_width_limit_stays_on_one_line_and_reparses
        PASS [   0.005s] ( 549/5472) aion-awl::printer_roundtrip trailing_comments_survive_and_normalize
        PASS [   0.020s] ( 550/5472) aion-awl::parser_corpus every_check_staged_invalid_fixture_parses_cleanly
        PASS [   0.020s] ( 551/5472) aion-awl::parser_corpus every_valid_fixture_parses
        PASS [   0.005s] ( 552/5472) aion-awl::printer_roundtrip wrapped_bind_chain_keeps_its_trailing_comment
        PASS [   0.022s] ( 553/5472) aion-awl::printer_roundtrip parse_print_is_identity_over_the_canonical_corpus
        PASS [   0.017s] ( 554/5472) aion-awl::query_surface both_words_stay_usable_as_ordinary_names
        PASS [   0.019s] ( 555/5472) aion-awl::query_surface queries_parse_only_when_declared
        PASS [   0.027s] ( 556/5472) aion-awl::query_surface both_backends_register_the_same_number_of_answers
        PASS [   0.023s] ( 557/5472) aion-awl::query_surface the_checker_refuses_each_broken_shape_and_accepts_its_repair
        PASS [   0.041s] ( 558/5472) aion-awl::query_surface both_backends_register_against_the_arguments_bearing_handler
        PASS [   0.021s] ( 559/5472) aion-awl::query_surface the_derived_schema_carries_the_query_surface
        PASS [   0.055s] ( 560/5472) aion-awl::printer_roundtrip print_parse_print_is_idempotent_over_the_whole_corpus
        PASS [   0.004s] ( 561/5472) aion-awl::recipe_compare_genericity a_ratchet_shaped_record_flows_through_the_compare_as_data
        PASS [   0.022s] ( 562/5472) aion-awl::query_surface the_direct_backend_lowers_one_handler_call_per_answer
        PASS [   0.005s] ( 563/5472) aion-awl::recipe_compare_genericity a_record_that_names_nothing_is_unrepresentable
        PASS [   0.004s] ( 564/5472) aion-awl::recipe_compare_genericity the_compare_consumes_every_member_of_the_vocabulary_uniformly
        PASS [   0.033s] ( 565/5472) aion-awl::query_surface the_compile_seam_carries_the_surface_into_the_beam
        PASS [   0.005s] ( 566/5472) aion-awl::recipe_compare_genericity the_ratchet_shaped_prior_art_is_cited_at_its_bytes
        PASS [   0.006s] ( 567/5472) aion-awl::recipe_compare_genericity the_compare_surface_matches_on_no_cause_identity
        PASS [   0.023s] ( 568/5472) aion-awl::query_surface the_printer_round_trips_the_surface
        PASS [   0.426s] ( 569/5472) aion-awl::emitter every_valid_fixture_emits
        PASS [   0.017s] ( 570/5472) aion-awl::recipe_compose a_bound_value_still_carrying_its_sentinel_refuses
        PASS [   0.030s] ( 571/5472) aion-awl::query_surface the_reference_backend_emits_one_handler_per_answer
        PASS [   0.017s] ( 572/5472) aion-awl::recipe_compose a_correction_propagates_to_every_consumer_with_a_sweep_record_listing_them
        PASS [   0.019s] ( 573/5472) aion-awl::recipe_compose a_declared_default_fills_a_slot_the_composition_omits
        PASS [   0.005s] ( 574/5472) aion-awl::recipe_compose the_sentinel_rule_reads_a_real_value_as_a_real_value
        PASS [   0.017s] ( 575/5472) aion-awl::recipe_compose a_source_no_recipe_names_is_swept_to_an_empty_list
        PASS [   0.234s] ( 576/5472) aion-awl::flow_vocab_roundtrip_property generated_vocabulary_documents_round_trip_losslessly
        PASS [   0.022s] ( 577/5472) aion-awl::recipe_compose a_filled_composition_resolves_and_defers_only_the_re_derive_slots
        PASS [   0.017s] ( 578/5472) aion-awl::recipe_compose an_unfilled_slot_refuses_with_the_slot_name_in_observed_bytes
        PASS [   0.013s] ( 579/5472) aion-awl::recipe_corpus every_exhibit_matches_the_digest_the_rig_pinned
        PASS [   0.009s] ( 580/5472) aion-awl::recipe_corpus the_fixture_set_carries_its_own_completeness_claim
        PASS [   0.020s] ( 581/5472) aion-awl::recipe_compose every_unfilled_slot_is_reported_at_once
        PASS [   0.006s] ( 582/5472) aion-awl::recipe_declarations a_command_declaration_cannot_be_nested_inside_a_recipe
        PASS [   0.004s] ( 583/5472) aion-awl::recipe_declarations a_cyclic_dependency_is_a_check_error_with_a_span
        PASS [   0.004s] ( 584/5472) aion-awl::recipe_declarations a_shell_string_step_body_is_unparseable
        PASS [   0.021s] ( 585/5472) aion-awl::recipe_corpus the_haem_wall_allowlist_is_one_declaration_and_its_projections_are_measured
        PASS [   0.022s] ( 586/5472) aion-awl::recipe_corpus the_ground_sentinel_guard_is_expressed_as_a_declared_artifact
        PASS [   0.019s] ( 587/5472) aion-awl::recipe_corpus the_staging_checklist_becomes_step_kinds_with_its_exclusions_named
        PASS [   0.004s] ( 588/5472) aion-awl::recipe_declarations every_assembly_form_still_refuses_inside_a_command_declaration
        PASS [   0.003s] ( 589/5472) aion-awl::recipe_declarations the_parameter_kind_taxonomy_is_closed
        PASS [   0.029s] ( 590/5472) aion-awl::recipe_corpus the_field_kind_rows_become_parameter_kind_declarations
        PASS [   0.026s] ( 591/5472) aion-awl::recipe_corpus the_kit_sed_fill_slots_become_declared_parameters
        PASS [   0.023s] ( 592/5472) aion-awl::recipe_corpus the_two_build_input_copies_differ_and_both_are_expressed
        PASS [   0.004s] ( 593/5472) aion-awl::recipe_fire a_receipt_that_cannot_say_what_it_measured_is_unrepresentable
        PASS [   0.270s] ( 594/5472) aion-awl::flow_vocab_roundtrip_property generated_flow_shape_documents_round_trip_losslessly
        PASS [   0.019s] ( 595/5472) aion-awl::recipe_declarations the_edges_and_not_the_source_order_decide_what_runs_first
        PASS [   0.021s] ( 596/5472) aion-awl::recipe_declarations the_assembly_surface_round_trips_through_the_canonical_printer
        PASS [   0.019s] ( 597/5472) aion-awl::recipe_fire a_clean_census_proceeds_and_the_receipt_lists_what_was_measured
        PASS [   0.027s] ( 598/5472) aion-awl::recipe_declarations the_three_step_bodies_parse_into_their_typed_forms
        PASS [   0.026s] ( 599/5472) aion-awl::recipe_fire a_census_mismatch_refuses_the_fire_with_both_hashes
        PASS [   0.026s] ( 600/5472) aion-awl::recipe_fire a_declared_command_that_fails_stops_the_stage
        PASS [   0.023s] ( 601/5472) aion-awl::recipe_fire a_runs_command_step_hands_the_emitted_argv_to_the_runner
        PASS [   0.028s] ( 602/5472) aion-awl::recipe_fire a_recipe_may_consume_a_dispatch_value_and_may_not_restate_it
        PASS [   0.023s] ( 603/5472) aion-awl::recipe_fire a_specimen_fire_against_the_known_answer_red_passes_and_its_receipt_carries_the_comparison
        PASS [   0.006s] ( 604/5472) aion-awl::recipe_fire an_object_that_moved_between_the_write_and_the_claim_refuses
        PASS [   0.026s] ( 605/5472) aion-awl::recipe_fire a_stage_compare_mismatch_refuses_with_both_sides_bytes
        PASS [   0.009s] ( 606/5472) aion-awl::recipe_fire an_object_that_stayed_put_is_claimed
        PASS [   0.026s] ( 607/5472) aion-awl::recipe_fire an_artifact_still_carrying_its_sentinel_refuses_at_the_fire_minute
        PASS [   0.034s] ( 608/5472) aion-awl::recipe_fire a_step_naming_an_undeclared_command_refuses_at_the_fire_layer
        PASS [   0.029s] ( 609/5472) aion-awl::recipe_fire an_artifact_with_no_declared_digest_cannot_pass_the_census
        PASS [   0.029s] ( 610/5472) aion-awl::recipe_fire an_inexact_restoration_refuses
        PASS [   0.014s] ( 611/5472) aion-awl::recipe_fire the_recipe_family_opens_no_file_and_spawns_no_process
        PASS [   0.007s] ( 612/5472) aion-awl::recipe_parameters a_declaration_carrying_the_same_truth_twice_is_a_check_error
        PASS [   0.007s] ( 613/5472) aion-awl::recipe_parameters a_derivation_on_a_non_re_derive_kind_is_a_check_error
        PASS [   0.030s] ( 614/5472) aion-awl::recipe_fire an_unanswered_precondition_stops_the_fire
        PASS [   0.008s] ( 615/5472) aion-awl::recipe_parameters a_re_derive_parameter_without_a_derivation_is_a_check_error
        PASS [   0.008s] ( 616/5472) aion-awl::recipe_parameters an_extraction_outside_the_closed_set_is_a_check_error
        PASS [   0.032s] ( 617/5472) aion-awl::recipe_fire an_unmeasurable_census_row_refuses_the_fire
        PASS [   0.030s] ( 618/5472) aion-awl::recipe_fire the_census_is_the_set_of_referenced_artifacts_named_once
        PASS [   0.006s] ( 619/5472) aion-awl::recipe_parameters the_extraction_grammar_reads_exactly_two_forms
        PASS [   0.040s] ( 620/5472) aion-awl::recipe_fire this_cut_adds_no_venue_binding_hole_beyond_the_one_cut_one_admitted
        PASS [   0.036s] ( 621/5472) aion-awl::recipe_parameters a_derivation_resolves_at_fire_time_against_the_named_source
        PASS [   0.031s] ( 622/5472) aion-awl::recipe_parameters an_extraction_past_the_end_of_its_source_refuses
        PASS [   0.029s] ( 623/5472) aion-awl::recipe_parameters composing_a_re_derive_slot_refuses_with_the_consumed_cause
        PASS [   0.030s] ( 624/5472) aion-awl::recipe_parameters each_declared_kind_parses_and_checks
        PASS [   0.007s] ( 625/5472) aion-awl::recipe_wall a_wall_that_allows_nothing_is_a_check_error
        PASS [   0.032s] ( 626/5472) aion-awl::recipe_parameters literal_where_derived_fires_against_a_re_derive_slot_fed_a_literal
        PASS [   0.035s] ( 627/5472) aion-awl::recipe_parameters an_unreadable_derivation_source_refuses_at_the_fire_layer
        PASS [   0.034s] ( 628/5472) aion-awl::recipe_wall a_diff_against_an_unprojectable_state_is_the_projection_refusal
        PASS [   0.026s] ( 629/5472) aion-awl::recipe_wall a_projection_that_would_emit_an_excluded_term_refuses
        PASS [   0.029s] ( 630/5472) aion-awl::recipe_wall one_wall_declaration_projects_into_both_renderings
        PASS [   0.036s] ( 631/5472) aion-awl::recipe_wall a_wall_whose_exclusions_are_absent_from_it_projects
        PASS [   0.032s] ( 632/5472) aion-awl::recipe_wall the_enforcement_rendering_escapes_accidents_and_keeps_intent
        PASS [   0.029s] ( 633/5472) aion-awl::recipe_wall two_identical_declaration_states_diff_to_nothing
        PASS [   0.035s] ( 634/5472) aion-awl::recipe_wall the_structured_diff_names_every_changed_projection
        PASS [   0.043s] ( 635/5472) aion-awl::recipe_wall an_excluded_term_in_the_walls_own_name_refuses
        PASS [   0.153s] ( 636/5472) aion-awl::recipe_declarations a_deep_chain_orders_rather_than_overflowing
        PASS [   0.033s] ( 637/5472) aion-awl::retry_counting declared_count_is_the_identity_and_does_not_move
        PASS [   0.042s] ( 638/5472) aion-awl::retry_counting direct_lowering_maps_three_further_attempts_to_four_total
        PASS [   0.038s] ( 639/5472) aion-awl::retry_counting reference_emitter_maps_three_further_attempts_to_four_total
        PASS [   0.035s] ( 640/5472) aion-awl::retry_counting zero_further_attempts_is_one_total_execution
        PASS [   0.054s] ( 641/5472) aion-awl::runtime_codecs assert_list_badmatch_reports_the_subject_at_runtime
        PASS [   0.046s] ( 642/5472) aion-awl::runtime_codecs fix_cycle::empty_list_literal_materializes_nil_at_runtime
        PASS [   2.669s] ( 643/5472) aion-awl mir::select::inspect_tests::covered_ratchet_upholds_ir14_and_section_11
        PASS [   2.538s] ( 644/5472) aion-awl::emitter compile_tests::signal_sender_fixture_compiles_under_gleam
        PASS [   2.646s] ( 645/5472) aion-awl::emitter compile_tests::flagship_fixtures_compile_under_gleam
        PASS [   2.703s] ( 646/5472) aion-awl::flow_vocab_compile_proof flow_vocab_b1_fixture_compiles_under_gleam
        PASS [   0.016s] ( 647/5472) aion-awl::runtime_codecs gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.010s] ( 648/5472) aion-awl::runtime_codecs gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.010s] ( 649/5472) aion-awl::runtime_codecs gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.010s] ( 650/5472) aion-awl::runtime_codecs gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.009s] ( 651/5472) aion-awl::runtime_codecs gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.016s] ( 652/5472) aion-awl::runtime_codecs gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   3.211s] ( 653/5472) aion-awl::emitter compile_tests::loop_and_fork_fixtures_compile_under_gleam
        PASS [   3.545s] ( 654/5472) aion-awl::emitter compile_tests::query_surface_fixture_compiles_under_gleam
        PASS [   3.299s] ( 655/5472) aion-awl::runtime_codecs fix_cycle::dev_brief_fix_cycle_encode_reaches_dispatch_with_reference_parity
        PASS [   3.376s] ( 656/5472) aion-awl::runtime_codecs direct_codecs_execute_with_reference_parity
        PASS [   0.069s] ( 657/5472) aion-awl::runtime_codecs semantic_pins::value_boolean_operators_skip_faulting_rhs_at_runtime
        PASS [   4.005s] ( 658/5472) aion-awl::flow_vocab_b4_compile_proof b4_flow_shape_fixtures_compile_under_gleam
        PASS [   3.605s] ( 659/5472) aion-awl::runtime_codecs concat::general_concat_executes_with_emitter_byte_parity
        PASS [   3.961s] ( 660/5472) aion-awl::runtime_codecs fork_generality::named_fork_heterogeneous_site_config_parity
        PASS [   4.014s] ( 661/5472) aion-awl::runtime_codecs fork_generality::named_fork_homogeneous_site_config_parity
        PASS [   0.034s] ( 662/5472) aion-awl::schema a_door_the_validator_cannot_compile_is_refused_at_check
        PASS [   0.032s] ( 663/5472) aion-awl::schema an_imported_schemas_internal_refs_survive_composition
        PASS [   0.027s] ( 664/5472) aion-awl::schema builtins_lists_and_optionals_map_to_json_types
        PASS [   0.033s] ( 665/5472) aion-awl::schema check_proves_the_signal_child_and_activity_schemas_too
        PASS [   0.026s] ( 666/5472) aion-awl::schema doc_comments_flow_to_descriptions_at_type_and_field_level
        PASS [   0.029s] ( 667/5472) aion-awl::schema enums_derive_string_enum_schemas
        PASS [   0.016s] ( 668/5472) aion-awl::schema import_derivation_without_a_root_refuses
        PASS [   0.035s] ( 669/5472) aion-awl::schema imported_schemas_reemit_verbatim_with_constraints_preserved
        PASS [   0.034s] ( 670/5472) aion-awl::schema inline_schemas_reemit_their_pasted_json_with_constraints
        PASS [   0.033s] ( 671/5472) aion-awl::schema named_type_references_become_local_defs
        PASS [   0.029s] ( 672/5472) aion-awl::schema optional_fields_are_omitted_from_required_and_never_nullable
        PASS [   0.050s] ( 673/5472) aion-awl::schema recursive_records_self_reference_the_schema_root
        PASS [   0.010s] ( 674/5472) aion-awl::schema unknown_types_are_refused_by_name
        PASS [   0.032s] ( 675/5472) aion-awl::schema workflow_contract_covers_inputs_with_optionality_and_narration
        PASS [   0.013s] ( 676/5472) aion-awl::schema_door brace_on_the_line_after_schema_is_not_a_raw_door
        PASS [   0.018s] ( 677/5472) aion-awl::schema_door braces_and_escaped_quotes_inside_json_strings_do_not_close_the_body
        PASS [   0.014s] ( 678/5472) aion-awl::schema_door inline_schema_body_is_one_verbatim_token
        PASS [   0.008s] ( 679/5472) aion-awl::schema_door negative_numbers_exponents_and_json_escapes_pass_verbatim
        PASS [   0.006s] ( 680/5472) aion-awl::schema_door no_structural_tokens_are_emitted_inside_a_multiline_body
        PASS [   0.006s] ( 681/5472) aion-awl::schema_door pasted_schema_with_non_two_space_indentation_lexes
        PASS [   0.009s] ( 682/5472) aion-awl::schema_door schema_body_span_is_document_true
        PASS [   0.009s] ( 683/5472) aion-awl::schema_door schema_file_import_door_is_not_raw_captured
        PASS [   0.006s] ( 684/5472) aion-awl::schema_door single_line_schema_body_keeps_lexing_the_rest_of_the_line
        PASS [   0.006s] ( 685/5472) aion-awl::schema_door tokens_after_a_multiline_body_report_the_closing_line
        PASS [   0.011s] ( 686/5472) aion-awl::schema_door unterminated_schema_body_reports_the_opening_brace
        PASS [   4.604s] ( 687/5472) aion-awl::runtime_codecs fork_generality::fork_inside_loop_completes_two_passes_with_reference_parity
        PASS [   0.071s] ( 688/5472) aion-awl::semantic semantic_spans_cover_declarations_and_checker_owned_scopes
        PASS [   0.056s] ( 689/5472) aion-awl::shell_run_example the_example_command_is_identity_bound
        PASS [   0.062s] ( 690/5472) aion-awl::shell_run_example the_shell_run_example_checks_clean_and_is_canonically_formatted
        PASS [   0.069s] ( 691/5472) aion-awl::shell_run_example the_shell_run_example_compiles_with_its_command_in_the_package_contract
        PASS [   0.031s] ( 692/5472) aion-awl::short_circuit gleam_emitter_keeps_optional_rhs_inside_some_branch
        PASS [   0.010s] ( 693/5472) aion-awl::signal_sender a_duplicate_outbound_declaration_is_refused
        PASS [   0.012s] ( 694/5472) aion-awl::signal_sender a_missing_payload_field_is_refused
        PASS [   4.811s] ( 695/5472) aion-awl::runtime_codecs child_envelope::real_child_outcome_envelope_completes_parent_fold_on_both_backends
        PASS [   0.057s] ( 696/5472) aion-awl::signal_sender a_document_without_sends_is_unaffected
        PASS [   0.015s] ( 697/5472) aion-awl::signal_sender a_non_string_target_is_refused
        PASS [   0.017s] ( 698/5472) aion-awl::signal_sender a_send_is_not_an_action_body
        PASS [   0.025s] ( 699/5472) aion-awl::signal_sender a_payload_field_of_the_wrong_type_is_refused
        PASS [   0.013s] ( 700/5472) aion-awl::signal_sender a_send_without_a_target_is_named
        PASS [   0.008s] ( 701/5472) aion-awl::signal_sender an_outbound_name_must_be_snake_case
        PASS [   0.048s] ( 702/5472) aion-awl::signal_sender a_send_is_legal_inside_an_on_failure_block
        PASS [   0.031s] ( 703/5472) aion-awl::signal_sender a_well_formed_send_parses_checks_and_round_trips
        PASS [   0.012s] ( 704/5472) aion-awl::signal_sender named_fields_on_a_leaf_payload_are_refused
        PASS [   0.016s] ( 705/5472) aion-awl::signal_sender sending_a_receive_only_signal_names_the_direction
        PASS [   0.010s] ( 706/5472) aion-awl::signal_sender sending_an_undeclared_signal_is_a_check_error
        PASS [   0.012s] ( 707/5472) aion-awl::signal_sender the_bare_form_is_refused_for_a_non_nil_payload
        PASS [   0.049s] ( 708/5472) aion-awl::signal_sender one_name_may_be_declared_inbound_and_outbound
        PASS [   0.046s] ( 709/5472) aion-awl::signal_sender send_is_not_reserved_and_still_names_an_action
        PASS [   0.017s] ( 710/5472) aion-awl::signal_sender the_corpus_fixture_is_already_canonical
        PASS [   0.016s] ( 711/5472) aion-awl::signal_sender the_payload_after_the_target_gets_a_fix_it
        PASS [   0.039s] ( 712/5472) aion-awl::worker_documents a_worker_document_checks_clean_and_prints_canonically
        PASS [   0.007s] ( 713/5472) aion-awl::worker_documents a_worker_document_publishes_no_start_contract
        PASS [   0.051s] ( 714/5472) aion-awl::signal_sender the_reference_backend_emits_signal_send
        PASS [   0.064s] ( 715/5472) aion-awl::signal_sender the_direct_backend_reaches_the_same_sdk_entry_point
        PASS [   0.009s] ( 716/5472) aion-awl::worker_documents command_hygiene_holds_in_the_worker_family
        PASS [   0.014s] ( 717/5472) aion-awl::worker_documents every_action_body_carries_its_capture
        PASS [   0.013s] ( 718/5472) aion-awl::worker_documents printing_normalizes_and_reparses
        PASS [   0.007s] ( 719/5472) aion-awl::worker_documents the_opening_declaration_chooses_the_family
        PASS [   0.008s] ( 720/5472) aion-awl::worker_documents the_worker_compiler_refuses_a_workflow_document_by_name
        PASS [   0.009s] ( 721/5472) aion-awl::worker_documents the_workflow_compiler_refuses_a_worker_document_by_name
        PASS [   0.038s] ( 722/5472) aion-awl::worker_documents the_advertised_contract_stays_worker_owed
        PASS [   0.012s] ( 723/5472) aion-awl::workflow_timeout overflowing_workflow_timeout_is_checked_at_the_declaration
        PASS [   0.042s] ( 724/5472) aion-awl::worker_documents the_advertised_surface_matches_the_callers_contract
        PASS [   0.013s] ( 725/5472) aion-awl::workflow_timeout workflow_timeout_parses_and_prints_with_header_trivia
        PASS [   0.009s] ( 726/5472) aion-awl::workflow_timeout zero_and_negative_workflow_timeouts_are_checked_at_the_declaration
        PASS [   0.009s] ( 727/5472) aion-awl-lsp analysis::tests::positions_are_utf16_not_bytes_or_scalar_columns
        PASS [   0.010s] ( 728/5472) aion-awl-lsp analysis::tests::point_spans_stay_points
        PASS [   0.034s] ( 729/5472) aion-awl-lsp bindings::bindings_tests::a_collision_with_the_same_kind_is_refused
        PASS [   0.065s] ( 730/5472) aion-awl::workflow_timeout timeout_is_compile_metadata_for_both_backends_not_workflow_code
        PASS [   0.046s] ( 731/5472) aion-awl-lsp bindings::bindings_tests::a_different_declaration_sharing_the_name_is_untouched
        PASS [   0.036s] ( 732/5472) aion-awl-lsp bindings::bindings_tests::a_name_of_the_wrong_case_class_is_refused
        PASS [   0.042s] ( 733/5472) aion-awl-lsp bindings::bindings_tests::a_keyword_binds_nothing_and_highlights_nothing
        PASS [   0.051s] ( 734/5472) aion-awl-lsp bindings::bindings_tests::a_position_with_no_declaration_is_refused
        PASS [   0.051s] ( 735/5472) aion-awl-lsp bindings::bindings_tests::a_reserved_keyword_is_refused
        PASS [   0.034s] ( 736/5472) aion-awl-lsp bindings::bindings_tests::prepare_selects_the_declaration_name
        PASS [   0.057s] ( 737/5472) aion-awl-lsp bindings::bindings_tests::highlighting_does_not_light_a_different_declaration_of_the_same_name
        PASS [   0.032s] ( 738/5472) aion-awl-lsp bindings::bindings_tests::renaming_a_type_rewrites_its_uses
        PASS [   0.036s] ( 739/5472) aion-awl-lsp bindings::bindings_tests::renaming_an_input_rewrites_every_use_and_leaves_a_checking_document
        PASS [   0.045s] ( 740/5472) aion-awl-lsp bindings::bindings_tests::references_find_every_use_and_honour_the_declaration_flag
        PASS [   0.034s] ( 741/5472) aion-awl-lsp bindings::bindings_tests::the_refusals_are_not_a_provider_that_refuses_everything
        PASS [   0.042s] ( 742/5472) aion-awl-lsp bindings::bindings_tests::the_highlight_set_is_exactly_what_a_rename_would_edit
        PASS [   0.009s] ( 743/5472) aion-awl-lsp completion::completion_tests::a_pipe_offers_its_stages
        PASS [   0.007s] ( 744/5472) aion-awl-lsp completion::completion_tests::a_type_position_offers_declared_and_builtin_types
        PASS [   0.007s] ( 745/5472) aion-awl-lsp completion::completion_tests::bindings_produced_by_a_statement_are_offered
        PASS [   0.060s] ( 746/5472) aion-awl-lsp bindings::bindings_tests::the_declaration_highlights_as_a_write_and_its_uses_as_reads
        PASS [   0.010s] ( 747/5472) aion-awl-lsp completion::completion_tests::declared_names_are_offered_while_the_document_does_not_parse
        PASS [   0.009s] ( 748/5472) aion-awl-lsp completion::completion_tests::declared_names_sort_above_keywords
        PASS [   0.007s] ( 749/5472) aion-awl-lsp completion::completion_tests::declared_names_survive_a_line_that_cannot_be_lexed
        PASS [   0.009s] ( 750/5472) aion-awl-lsp completion::completion_tests::every_item_is_classified
        PASS [   0.013s] ( 751/5472) aion-awl-lsp completion::completion_tests::every_keyword_is_offered_somewhere
        PASS [   0.007s] ( 752/5472) aion-awl-lsp completion::completion_tests::route_offers_outcomes_and_steps_not_statement_openers
        PASS [   0.013s] ( 753/5472) aion-awl-lsp completion::completion_tests::every_offering_position_is_reachable_through_completions
        PASS [   0.007s] ( 754/5472) aion-awl-lsp completion::completion_tests::the_enclosing_block_decides_when_nothing_else_does
        PASS [   0.009s] ( 755/5472) aion-awl-lsp completion::completion_tests::the_positional_words_are_offered_where_the_parser_accepts_them
        PASS [   0.008s] ( 756/5472) aion-awl-lsp completion::completion_tests::value_words_are_offered_where_authors_actually_write_them
        PASS [   0.006s] ( 757/5472) aion-awl-lsp completion::completion_tests::wait_offers_signals_only
        PASS [   0.007s] ( 758/5472) aion-awl-lsp glossary::glossary_tests::a_command_word_used_as_a_name_still_hovers_to_nothing
        PASS [   0.009s] ( 759/5472) aion-awl-lsp glossary::glossary_tests::a_clause_word_of_the_wrong_cluster_hovers_to_nothing
        PASS [   0.007s] ( 760/5472) aion-awl-lsp glossary::glossary_tests::a_keyword_answers_in_a_document_that_does_not_parse
        PASS [   0.007s] ( 761/5472) aion-awl-lsp glossary::glossary_tests::an_offered_word_carries_the_language_s_sentence
        PASS [   0.008s] ( 762/5472) aion-awl-lsp glossary::glossary_tests::completion_inside_a_command_body_offers_the_bodys_vocabulary
        PASS [   0.011s] ( 763/5472) aion-awl-lsp glossary::glossary_tests::every_command_word_hovers_to_the_guide_sentence_from_the_table
        PASS [   0.007s] ( 764/5472) aion-awl-lsp glossary::glossary_tests::every_offered_word_of_the_language_is_documented
        PASS [   0.008s] ( 765/5472) aion-awl-lsp glossary::glossary_tests::hover_and_completion_agree_and_completion_shows_one_example
        PASS [   0.012s] ( 766/5472) aion-awl-lsp glossary::glossary_tests::every_recipe_word_hovers_to_the_guide_sentence_from_the_table
        PASS [   0.010s] ( 767/5472) aion-awl-lsp glossary::glossary_tests::hover_makes_no_claim_about_a_word_that_might_be_a_name
        PASS [   0.046s] ( 768/5472) aion-awl-lsp glossary::glossary_tests::a_declared_name_still_wins_over_the_glossary
        PASS [   0.012s] ( 769/5472) aion-awl-lsp::protocol live_completion_answers_a_half_typed_step_body
        PASS [   0.038s] ( 770/5472) aion-awl-lsp::protocol live_document_highlight_distinguishes_the_declaration_from_its_uses
        PASS [   0.012s] ( 771/5472) aion-awl-lsp::protocol live_initialize_open_diagnose_format_shutdown_exchange
        PASS [   3.758s] ( 772/5472) aion-awl::runtime_codecs fork_generality::parallel_action_fork_site_config_dispatches_with_reference_parity
        PASS [   0.095s] ( 773/5472) aion-awl-lsp::corpus lsp_g2_all_valid_fixtures_use_the_canonical_idempotent_printer
        PASS [   5.471s] ( 774/5472) aion-awl::runtime_codecs child_envelope::child_output_codec_is_strict_and_symmetric_on_both_backends
        PASS [   0.081s] ( 775/5472) aion-awl-lsp::protocol live_references_and_rename_including_a_refusal
        PASS [   0.007s] ( 776/5472) aion-awl-package bundle::tests::closure_is_nonempty_and_versioned
        PASS [   0.060s] ( 777/5472) aion-awl-lsp::protocol rev2_corpus_hover_and_definition_use_checker_semantics
        PASS [   0.006s] ( 778/5472) aion-awl-package bundle::tests::generated_table_is_sorted_and_duplicate_free
        PASS [   0.064s] ( 779/5472) aion-awl-package assemble::tests::repeated_action_requirements_dedupe_into_one_activity
        PASS [   0.007s] ( 780/5472) aion-awl-package source::tests::a_current_directory_prefix_normalises_to_the_plain_path
        PASS [   0.010s] ( 781/5472) aion-awl-package source::tests::a_document_filename_with_a_directory_part_is_refused
        PASS [   0.009s] ( 782/5472) aion-awl-package source::tests::a_document_without_imports_collects_no_schemas
        PASS [   0.008s] ( 783/5472) aion-awl-package source::tests::an_import_above_the_document_root_is_refused
        PASS [   0.009s] ( 784/5472) aion-awl-package source::tests::an_unreadable_import_is_reported
        PASS [   0.010s] ( 785/5472) aion-awl-package source::tests::nested_schema_imports_keep_their_document_relative_paths
        PASS [   0.013s] ( 786/5472) aion-awl-package source::tests::sibling_schema_import_is_archived_beside_the_document
        PASS [   3.277s] ( 787/5472) aion-awl::runtime_codecs nested_collection_predicates_execute_with_reference_parity
        PASS [   0.044s] ( 788/5472) aion-awl-package::awl_assemble bundle_is_closed_under_imports_modulo_engine_and_erts_modules
        PASS [   0.131s] ( 789/5472) aion-awl-package assemble::tests::unauthored_timeout_is_absent_and_authored_timeout_is_declared
        PASS [   0.072s] ( 790/5472) aion-awl-package::awl_assemble bundle_is_versioned_pinned_and_carries_the_imported_sdk_modules
        PASS [   0.039s] ( 791/5472) aion-awl-package::awl_assemble every_workflow_beam_import_is_shipped_in_the_closure
        PASS [   0.008s] ( 792/5472) aion-awl-package::awl_assemble regeneration_script_is_documented_and_outside_the_product_path
        PASS [   0.009s] ( 793/5472) aion-awl-package::awl_assemble unsafe_workflow_name_is_refused_typed
        PASS [   0.006s] ( 794/5472) aion-awl-package::awl_assemble workflow_name_colliding_with_a_bundle_module_is_refused_typed
        PASS [   0.111s] ( 795/5472) aion-awl-package::awl_assemble assembling_twice_yields_byte_identical_archives
        PASS [   0.048s] ( 796/5472) aion-awl-package::awl_assemble fixed_input_archive_bytes_match_the_cross_process_pin
        PASS [   0.083s] ( 797/5472) aion-awl-package::awl_assemble format_v1_loader_admits_the_archive_with_derived_manifest
        PASS [   0.082s] ( 798/5472) aion-awl-package::awl_source_round_trip a_document_without_imports_still_carries_its_own_source
        PASS [   2.812s] ( 799/5472) aion-awl::runtime_codecs pipes::pipe_combinators_execute_with_reference_parity
        PASS [   0.125s] ( 800/5472) aion-awl-package::awl_source_round_trip a_sibling_schema_import_is_archived_beside_the_document
        PASS [   0.144s] ( 801/5472) aion-awl-package::awl_source_round_trip a_deployed_document_and_its_nested_schemas_survive_the_persistence_rewrite
        PASS [   0.440s] ( 802/5472) aion-awl-lsp::corpus lsp_g1_rev2_corpus_diagnostics_conform
        PASS [   3.879s] ( 803/5472) aion-awl::runtime_codecs fork_generality::plain_call_site_config_merges_per_key_with_reference_parity
        PASS [   0.243s] ( 804/5472) aion-awl-package::awl_assemble document_timeout_reaches_the_manifest_and_absence_arms_nothing
        PASS [   0.099s] ( 805/5472) aion-awl-package::declared_command_sealing an_assembled_archive_carries_the_declared_command_and_loads
        PASS [   0.157s] ( 806/5472) aion-awl-package::awl_source_round_trip assembly_with_archived_source_is_byte_deterministic
        PASS [   0.009s] ( 807/5472) aion-awl-package::distribute_parity duplicate_workflow_types_are_refused_typed
        PASS [   0.082s] ( 808/5472) aion-awl-package::distribute_parity direct_archive_carries_the_implicit_child_with_gleam_path_identity
        PASS [   0.006s] ( 809/5472) aion-awl-package::distribute_parity unsafe_synthesized_workflow_type_is_refused_typed
        PASS [   0.215s] ( 810/5472) aion-awl-package::awl_source_round_trip archiving_the_document_does_not_change_the_package_version
        PASS [   0.010s] ( 811/5472) aion-awl-package::sdk_closure_freshness a_changed_body_is_invisible_to_exports_and_caught_by_bytes
        PASS [   0.005s] ( 812/5472) aion-awl-package::sdk_closure_freshness a_changed_metadata_chunk_does_not_diverge
        PASS [   0.005s] ( 813/5472) aion-awl-package::sdk_closure_freshness byte_identical_beams_do_not_diverge
        PASS [   0.024s] ( 814/5472) aion-awl-package::runtime_surface_guard every_runtime_fn_signature_is_exported_by_the_closure
        PASS [   0.015s] ( 815/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   3.923s] ( 816/5472) aion-awl::runtime_codecs fork_generality::sequential_fork_site_config_and_index_prelude_dispatch_with_reference_parity
        PASS [   0.007s] ( 817/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.005s] ( 818/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.008s] ( 819/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.007s] ( 820/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.092s] ( 821/5472) aion-awl-package::distribute_parity direct_distribute_archive_is_deterministic
        PASS [   0.005s] ( 822/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.080s] ( 823/5472) aion-awl-package::distribute_parity single_activity_distribute_synthesizes_no_child
        PASS [   0.005s] ( 824/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.007s] ( 825/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.018s] ( 826/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.284s] ( 827/5472) aion-awl-package::declared_command_sealing repacking_without_editing_anything_still_loads
        PASS [   0.330s] ( 828/5472) aion-awl-package::declared_command_sealing a_one_character_command_edit_is_enough_to_refuse
        PASS [   0.320s] ( 829/5472) aion-awl-package::declared_command_sealing deleting_the_body_outright_makes_the_archive_refuse_to_load
        PASS [   0.039s] ( 830/5472) aion-cli::assistant_verb_e2e start_requires_the_operator_to_state_the_repository_question
        PASS [   0.306s] ( 831/5472) aion-awl-package::declared_command_sealing rewriting_the_declared_command_makes_the_archive_refuse_to_load
        PASS [   0.142s] ( 832/5472) aion-cli::assistant_verb_e2e a_valid_message_against_a_dead_endpoint_reports_a_connection_failure
        PASS [   0.013s] ( 833/5472) aion-cli::authoring_quality_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.004s] ( 834/5472) aion-cli::authoring_quality_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.006s] ( 835/5472) aion-cli::authoring_quality_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.007s] ( 836/5472) aion-cli::authoring_quality_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.036s] ( 837/5472) aion-cli::authoring_quality_e2e determinism_gate_passes_clean_and_fails_on_injected_wall_clock
        PASS [   0.005s] ( 838/5472) aion-cli::authoring_quality_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 839/5472) aion-cli::authoring_quality_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.175s] ( 840/5472) aion-cli::assistant_verb_e2e document_prints_the_embedded_document_verbatim
        PASS [   0.007s] ( 841/5472) aion-cli::authoring_quality_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.005s] ( 842/5472) aion-cli::authoring_quality_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.015s] ( 843/5472) aion-cli::authoring_quality_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.212s] ( 844/5472) aion-cli::assistant_verb_e2e an_empty_message_is_refused_before_any_connection
        PASS [   0.029s] ( 845/5472) aion-cli::awl_corpus_gate the_derived_corpus_is_non_vacuous_and_covers_every_area
        PASS [   0.018s] ( 846/5472) aion-cli::awl_corpus_gate the_gate_refuses_the_retired_three_field_agent_shape
        PASS [   0.014s] ( 847/5472) aion-cli::awl_emit_beam emit_beam_binary_refuses_stdout
        PASS [   0.270s] ( 848/5472) aion-cli::assistant_verb_e2e describe_answers_from_the_binary_with_no_server
        PASS [   0.039s] ( 849/5472) aion-cli::awl_recipe_verb_e2e a_fill_that_is_not_a_binding_is_refused
        PASS [   2.795s] ( 850/5472) aion-awl::runtime_codecs stmt_parity::index_and_lists_execute_with_reference_parity
        PASS [   0.121s] ( 851/5472) aion-cli::awl_emit_beam emit_beam_binary_output_equals_the_archive_entry_module
        PASS [   0.038s] ( 852/5472) aion-cli::awl_recipe_verb_e2e an_undeclared_recipe_is_named_with_what_the_document_does_declare
        PASS [   0.122s] ( 853/5472) aion-cli::awl_emit_beam emit_beam_resolves_schema_imports_from_the_document_directory
        PASS [   0.050s] ( 854/5472) aion-cli::awl_recipe_verb_e2e compose_refuses_an_unfilled_slot_by_name
        PASS [   0.012s] ( 855/5472) aion-cli::console_guide_asset the_console_asset_is_readable_json_with_the_shape_the_panel_uses
        PASS [   0.048s] ( 856/5472) aion-cli::awl_recipe_verb_e2e fire_measures_derives_fires_runs_and_writes_a_receipt
        PASS [   0.045s] ( 857/5472) aion-cli::awl_recipe_verb_e2e fire_refuses_a_census_mismatch_measured_against_the_real_tree
        PASS [   0.049s] ( 858/5472) aion-cli::awl_recipe_verb_e2e fire_refuses_an_unanswered_precondition_and_still_prints_its_receipt
        PASS [   0.321s] ( 859/5472) aion-cli::assistant_verb_e2e the_local_subcommands_never_need_a_server
        PASS [   0.035s] ( 860/5472) aion-cli::console_guide_asset the_console_asset_is_what_the_cli_renders_today
        PASS [   0.013s] ( 861/5472) aion-cli::deploy_commands gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.017s] ( 862/5472) aion-cli::deploy_commands gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.056s] ( 863/5472) aion-cli::declared_environment_report_e2e a_declaration_this_machine_can_satisfy_produces_no_warning_at_all
        PASS [   0.010s] ( 864/5472) aion-cli::deploy_commands gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.018s] ( 865/5472) aion-cli::deploy_commands gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.010s] ( 866/5472) aion-cli::deploy_commands gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.011s] ( 867/5472) aion-cli::deploy_commands gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.007s] ( 868/5472) aion-cli::deploy_commands gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.078s] ( 869/5472) aion-cli::declared_environment_report_e2e an_absent_declared_variable_is_reported_on_stderr_where_an_operator_reads_it
        PASS [   0.016s] ( 870/5472) aion-cli::deploy_commands gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.020s] ( 871/5472) aion-cli::deploy_commands gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.013s] ( 872/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.009s] ( 873/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.007s] ( 874/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   3.887s] ( 875/5472) aion-awl::runtime_codecs pipes::pipe_child_stage_executes_with_reference_parity
        PASS [   0.005s] ( 876/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.005s] ( 877/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.004s] ( 878/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.006s] ( 879/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.005s] ( 880/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.015s] ( 881/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.067s] ( 882/5472) aion-cli::describe_history_bound_e2e short_history_describe_is_unchanged
        PASS [   0.007s] ( 883/5472) aion-cli::doc_pointer_gate the_extractor_reads_both_pointer_forms_and_skips_what_is_not_a_path
        PASS [   0.009s] ( 884/5472) aion-cli::doc_pointer_gate the_gate_rejects_a_pointer_at_a_path_that_is_not_there
        PASS [   0.015s] ( 885/5472) aion-cli::doc_pointer_gate the_project_templates_still_point_somewhere
        PASS [   0.027s] ( 886/5472) aion-cli::doc_pointer_gate every_documentation_pointer_names_a_file_that_exists
        PASS [   3.258s] ( 887/5472) aion-awl::runtime_codecs semantic_pins::duration_boundary_executes_exactly_and_next_second_is_refused
        PASS [   0.008s] ( 888/5472) aion-cli::embedded_assistant_launchable_e2e the_spike_carries_the_embedded_prompt_constants_verbatim
        FAIL [   0.081s] ( 889/5472) aion-cli::embedded_assistant_launchable_e2e the_embedded_document_is_launchable_by_the_recommended_verb
  stdout ───

    running 1 test
    test the_embedded_document_is_launchable_by_the_recommended_verb ... FAILED

    failures:

    failures:
        the_embedded_document_is_launchable_by_the_recommended_verb

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.08s

  stderr ───

    thread 'the_embedded_document_is_launchable_by_the_recommended_verb' (2574296) panicked at crates/aion-cli/tests/embedded_assistant_launchable_e2e.rs:69:5:
    the server's QUEUE MOVE warn tells the operator to run `aion worker agent` on this document; the document must satisfy that verb as shipped (#209). stderr was:
    error: the `harness` section of /home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-cli/../aion-server/assistant-embed/assistant.awl sets `cwd` to `{workspace_root}`, which asks this box for its workspace root: the aion home cannot be resolved, so there is no workspace root: configuration error: cannot resolve Aion home: set AION_HOME or HOME

    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

  Cancelling due to test failure: 15 tests still running
        FAIL [   0.113s] ( 890/5472) aion-cli::embedded_assistant_launchable_e2e the_recommended_serve_invocation_reaches_the_dial_not_a_refusal
  stdout ───

    running 1 test
    test the_recommended_serve_invocation_reaches_the_dial_not_a_refusal ... FAILED

    failures:

    failures:
        the_recommended_serve_invocation_reaches_the_dial_not_a_refusal

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.11s

  stderr ───

    thread 'the_recommended_serve_invocation_reaches_the_dial_not_a_refusal' (2574323) panicked at crates/aion-cli/tests/embedded_assistant_launchable_e2e.rs:268:5:
    the recommended invocation must survive to the dial — an early exit is a serve-path refusal, which is the 0.14.0 defect; stderr was:
    error: the `harness` section of /home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-cli/../aion-server/assistant-embed/assistant.awl sets `cwd` to `{workspace_root}`, which asks this box for its workspace root: the aion home cannot be resolved, so there is no workspace root: configuration error: cannot resolve Aion home: set AION_HOME or HOME

    log was:

    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

        PASS [   0.202s] ( 891/5472) aion-cli::describe_history_bound_e2e describe_answers_for_histories_of_arbitrary_size
        PASS [   0.047s] ( 892/5472) aion-cli::error_rendering connect_failure_renders_unavailable_with_detail_and_hint
        PASS [   0.145s] ( 893/5472) aion-cli::embedded_assistant_launchable_e2e the_same_document_without_its_harness_section_is_refused
        PASS [   0.675s] ( 894/5472) aion-cli::assistant_verb_e2e the_status_verb_round_trips_against_a_live_server
        PASS [   3.514s] ( 895/5472) aion-awl::runtime_codecs stmt_parity::child_call_statement_and_spawn_execute_with_reference_parity
        PASS [   3.356s] ( 896/5472) aion-awl::runtime_codecs stmt_parity::wait_signal_binds_payload_with_reference_parity
        PASS [   6.998s] ( 897/5472) aion-awl::runtime_codecs fallible_predicates_execute_through_production_hosts_with_parity
        PASS [   0.692s] ( 898/5472) aion-cli::deploy_commands deploy_subcommands_drive_a_live_server
        PASS [   1.509s] ( 899/5472) aion-awl-package::sdk_closure_freshness committed_closure_matches_a_fresh_oracle_harvest
        PASS [   3.824s] ( 900/5472) aion-awl::runtime_codecs stmt_parity::wait_timeout_arms_execute_with_reference_parity
        PASS [   1.991s] ( 901/5472) aion-cli::awl_corpus_gate every_living_awl_document_checks_clean
        PASS [   2.322s] ( 902/5472) aion-cli::authoring_quality_e2e generated_test_scaffold_compiles_under_gleam_build
        PASS [   2.349s] ( 903/5472) aion-cli::authoring_quality_e2e input_skeleton_round_trips_through_the_workflow_input_codec
        PASS [   2.496s] ( 904/5472) aion-cli::dev_hot_reload_e2e dev_hot_loads_a_new_content_hash_version_on_edit_without_restart
────────────
     Summary [  10.044s] 904/5472 tests run: 902 passed, 2 failed, 0 skipped
        FAIL [   0.081s] ( 889/5472) aion-cli::embedded_assistant_launchable_e2e the_embedded_document_is_launchable_by_the_recommended_verb
        FAIL [   0.113s] ( 890/5472) aion-cli::embedded_assistant_launchable_e2e the_recommended_serve_invocation_reaches_the_dial_not_a_refusal
warning: 4568/5472 tests were not run due to test failure (run with --no-fail-fast to run all tests, or run with --max-fail)
error: test run failed
GATE-RED: cargo nextest run --workspace rc=100 (named cause: suite_red)
