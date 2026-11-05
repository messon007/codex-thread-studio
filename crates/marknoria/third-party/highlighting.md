# Syntax engine and grammar provenance

The production highlighter uses official `tree-sitter-highlight` 0.27.1 and its
Tree-sitter runtime (MIT, [license](tree-sitter-MIT.txt)). Grammars and queries are
pinned Arborium 2.18.2 crates, statically linked without Arborium's runtime fork.
The `crates/syntax` registry/alias table is adapted locally from Arborium 2.18.2's `src/store.rs`
(MIT OR Apache-2.0, [MIT notice](arborium-MIT.txt)). No runtime fetch is performed.

Upstream's [grammar license inventory](arborium-grammar-licenses.md) is retained.
NGINX (GPL) and Uiua (MPL) are excluded. The production registry has 110 grammars;
this inventory identifies their upstream revisions and licenses, not a private
or generated document fixture. The queried capture data remains in its pinned
crate. Non-color spelling/conceal captures are removed before compiling standard
Tree-sitter queries; INI newline mapping is implemented by MarkNoria.

| Crate | Version | Upstream license | Upstream revision | Upstream repository |
| --- | --- | --- | --- | --- |
| `arborium-ada` | 2.18.2 | MIT | Not recorded by package | [briot/tree-sitter-ada](https://github.com/briot/tree-sitter-ada) |
| `arborium-agda` | 2.18.2 | MIT | Not recorded by package | [tree-sitter/tree-sitter-agda](https://github.com/tree-sitter/tree-sitter-agda) |
| `arborium-asciidoc` | 2.18.2 | Apache-2.0 | `0ecc13042b7352ddc3f7b8b31432e262711d44e4` | [cathaysia/tree-sitter-asciidoc](https://github.com/cathaysia/tree-sitter-asciidoc) |
| `arborium-asm` | 2.18.2 | MIT | `839741fef4dab5128952334624905c82b40c7133` | [RubixDev/tree-sitter-asm](https://github.com/RubixDev/tree-sitter-asm) |
| `arborium-awk` | 2.18.2 | MIT | `34bbdc7cce8e803096f47b625979e34c1be38127` | [Beaglefoot/tree-sitter-awk](https://github.com/Beaglefoot/tree-sitter-awk) |
| `arborium-bash` | 2.18.2 | MIT | `20f55fb9663568f16ebceaac7673d2aa530add46` | [tree-sitter/tree-sitter-bash](https://github.com/tree-sitter/tree-sitter-bash) |
| `arborium-batch` | 2.18.2 | MIT | `737a031b42240bf61bf7ea5e4356d4e0580dd6d9` | [davidevofficial/tree-sitter-batch](https://github.com/davidevofficial/tree-sitter-batch) |
| `arborium-c` | 2.18.2 | MIT | `ae19b676b13bdcc13b7665397e6d9b14975473dd` | [tree-sitter/tree-sitter-c](https://github.com/tree-sitter/tree-sitter-c) |
| `arborium-c-sharp` | 2.18.2 | MIT | `485f0bae0274ac9114797fc10db6f7034e4086e3` | [tree-sitter/tree-sitter-c-sharp](https://github.com/tree-sitter/tree-sitter-c-sharp) |
| `arborium-caddy` | 2.18.2 | MIT | `65b60437983933d00809c8927e7d8a29ca26dfa3` | [Samonitari/tree-sitter-caddy](https://github.com/Samonitari/tree-sitter-caddy) |
| `arborium-capnp` | 2.18.2 | MIT | `7b0883c03e5edd34ef7bcf703194204299d7099f` | [tree-sitter-grammars/tree-sitter-capnp](https://github.com/tree-sitter-grammars/tree-sitter-capnp) |
| `arborium-cedar` | 2.18.2 | MIT OR Apache-2.0 | `f4be518f20f71ee62eaff4f3f1de07254c240704` | [DuskSystems/tree-sitter-cedar](https://github.com/DuskSystems/tree-sitter-cedar) |
| `arborium-cedarschema` | 2.18.2 | MIT OR Apache-2.0 | `f4be518f20f71ee62eaff4f3f1de07254c240704` | [DuskSystems/tree-sitter-cedar](https://github.com/DuskSystems/tree-sitter-cedar) |
| `arborium-clojure` | 2.18.2 | Unlicense | `e43eff80d17cf34852dcd92ca5e6986d23a7040f` | [sogaiu/tree-sitter-clojure](https://github.com/sogaiu/tree-sitter-clojure) |
| `arborium-cmake` | 2.18.2 | MIT | Not recorded by package | [uyha/tree-sitter-cmake](https://github.com/uyha/tree-sitter-cmake) |
| `arborium-cobol` | 2.18.2 | MIT | `befb226951b65585617f42ecbe526d00c124100c` | [zharinov/tree-sitter-cobol](https://github.com/zharinov/tree-sitter-cobol) |
| `arborium-commonlisp` | 2.18.2 | MIT | Not recorded by package | [theHamsta/tree-sitter-commonlisp](https://github.com/theHamsta/tree-sitter-commonlisp) |
| `arborium-cpp` | 2.18.2 | MIT | `12bd6f7e96080d2e70ec51d4068f2f66120dde35` | [tree-sitter/tree-sitter-cpp](https://github.com/tree-sitter/tree-sitter-cpp) |
| `arborium-css` | 2.18.2 | MIT | `dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f` | [tree-sitter/tree-sitter-css](https://github.com/tree-sitter/tree-sitter-css) |
| `arborium-d` | 2.18.2 | MIT | Not recorded by package | [gdamore/tree-sitter-d](https://github.com/gdamore/tree-sitter-d) |
| `arborium-dart` | 2.18.2 | MIT | `d4d8f3e337d8be23be27ffc35a0aef972343cd54` | [UserNobody14/tree-sitter-dart](https://github.com/UserNobody14/tree-sitter-dart) |
| `arborium-devicetree` | 2.18.2 | MIT | `e685f1f6ac1702b046415efb476444167d63e41` | [joelspadin/tree-sitter-devicetree](https://github.com/joelspadin/tree-sitter-devicetree) |
| `arborium-diff` | 2.18.2 | MIT | `2520c3f934b3179bb540d23e0ef45f75304b5fed` | [the-mikedavis/tree-sitter-diff](https://github.com/the-mikedavis/tree-sitter-diff) |
| `arborium-dockerfile` | 2.18.2 | MIT | `971acdd908568b4531b0ba28a445bf0bb720aba5` | [camdencheek/tree-sitter-dockerfile](https://github.com/camdencheek/tree-sitter-dockerfile) |
| `arborium-dot` | 2.18.2 | MIT | Not recorded by package | [rydesun/tree-sitter-dot](https://github.com/rydesun/tree-sitter-dot) |
| `arborium-elisp` | 2.18.2 | MIT | Not recorded by package | [Wilfred/tree-sitter-elisp](https://github.com/Wilfred/tree-sitter-elisp) |
| `arborium-elixir` | 2.18.2 | Apache-2.0 | `d24cecee673c4c770f797bac6f87ae4b6d7ddec5` | [elixir-lang/tree-sitter-elixir](https://github.com/elixir-lang/tree-sitter-elixir) |
| `arborium-elm` | 2.18.2 | MIT | `d0bec479c0422ed5eb01467790bd3912142fe987` | [elm-tooling/tree-sitter-elm](https://github.com/elm-tooling/tree-sitter-elm) |
| `arborium-erlang` | 2.18.2 | Apache-2.0 | Not recorded by package | [WhatsApp/tree-sitter-erlang](https://github.com/WhatsApp/tree-sitter-erlang) |
| `arborium-fish` | 2.18.2 | Unlicense | Not recorded by package | [ram02z/tree-sitter-fish](https://github.com/ram02z/tree-sitter-fish) |
| `arborium-fsharp` | 2.18.2 | MIT | `5141851c278a99958469eb1736c7afc4ec738e47` | [ionide/tree-sitter-fsharp](https://github.com/ionide/tree-sitter-fsharp) |
| `arborium-gitattributes` | 2.18.2 | MIT | `1b7af09d45b579f9f288453b95ad555f1f431645` | [tree-sitter-grammars/tree-sitter-gitattributes](https://github.com/tree-sitter-grammars/tree-sitter-gitattributes) |
| `arborium-gleam` | 2.18.2 | Apache-2.0 | `f4d55f3c5b690d4afe8853da892bc290d96deb34` | [gleam-lang/tree-sitter-gleam](https://github.com/gleam-lang/tree-sitter-gleam) |
| `arborium-glsl` | 2.18.2 | MIT | `24a6c8ef698e4480fecf8340d771fbcb5de8fbb4` | [tree-sitter-grammars/tree-sitter-glsl](https://github.com/tree-sitter-grammars/tree-sitter-glsl) |
| `arborium-go` | 2.18.2 | MIT | `2346a3ab1bb3857b48b29d779a1ef9799a248cd7` | [tree-sitter/tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) |
| `arborium-graphql` | 2.18.2 | MIT | Not recorded by package | [bkegley/tree-sitter-graphql](https://github.com/bkegley/tree-sitter-graphql) |
| `arborium-groovy` | 2.18.2 | MIT | `86911590a8e46d71301c66468e5620d9faa5b6af` | [murtaza64/tree-sitter-groovy](https://github.com/murtaza64/tree-sitter-groovy) |
| `arborium-haskell` | 2.18.2 | MIT | `98aedbd2d6947a168ba3ba3755d70b0cb6b78395` | [tree-sitter-grammars/tree-sitter-haskell](https://github.com/tree-sitter-grammars/tree-sitter-haskell) |
| `arborium-hcl` | 2.18.2 | Apache-2.0 | `fad991865fee927dd1de5e172fb3f08ac674d914` | [tree-sitter-grammars/tree-sitter-hcl](https://github.com/tree-sitter-grammars/tree-sitter-hcl) |
| `arborium-hlsl` | 2.18.2 | MIT | `bab9111922d53d43668fabb61869bec51bbcb915` | [tree-sitter-grammars/tree-sitter-hlsl](https://github.com/tree-sitter-grammars/tree-sitter-hlsl) |
| `arborium-html` | 2.18.2 | MIT | `73a3947324f6efddf9e17c0ea58d454843590cc0` | [tree-sitter/tree-sitter-html](https://github.com/tree-sitter/tree-sitter-html) |
| `arborium-idris` | 2.18.2 | MIT | `c56a25cf57c68ff929356db25505c1cc4c7820f6` | [kayhide/tree-sitter-idris](https://github.com/kayhide/tree-sitter-idris) |
| `arborium-ini` | 2.18.2 | Apache-2.0 | `bea223734aa9cea2c8793bf9098bd7fdef912f57` | [justinmk/tree-sitter-ini](https://github.com/justinmk/tree-sitter-ini) |
| `arborium-java` | 2.18.2 | MIT | `e10607b45ff745f5f876bfa3e94fbcc6b44bdc11` | [tree-sitter/tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) |
| `arborium-javascript` | 2.18.2 | MIT | `58404d8cf191d69f2674a8fd507bd5776f46cb11` | [tree-sitter/tree-sitter-javascript](https://github.com/tree-sitter/tree-sitter-javascript) |
| `arborium-jinja2` | 2.18.2 | Apache-2.0 | `922b28e1352c4966418b2e7e3772e0583d4532a2` | [dbt-labs/tree-sitter-jinja2](https://github.com/dbt-labs/tree-sitter-jinja2) |
| `arborium-jq` | 2.18.2 | MIT | `c204e36d2c3c6fce1f57950b12cabcc24e5cc4d9` | [flurie/tree-sitter-jq](https://github.com/flurie/tree-sitter-jq) |
| `arborium-jsdoc` | 2.18.2 | MIT | `658d18dcdddb75c760363faa4963427a7c6b52db` | [tree-sitter/tree-sitter-jsdoc](https://github.com/tree-sitter/tree-sitter-jsdoc) |
| `arborium-json` | 2.18.2 | MIT | `001c28d7a29832b06b0e831ec77845553c89b56d` | [tree-sitter/tree-sitter-json](https://github.com/tree-sitter/tree-sitter-json) |
| `arborium-julia` | 2.18.2 | MIT | Not recorded by package | [tree-sitter/tree-sitter-julia](https://github.com/tree-sitter/tree-sitter-julia) |
| `arborium-just` | 2.18.2 | MIT | `5685543a6e64f66335e25518c9ae8ffa1dae3d01` | [casey/tree-sitter-just](https://github.com/casey/tree-sitter-just) |
| `arborium-kconfig` | 2.18.2 | MIT | `9ac99fe4c0c27a35dc6f757cef534c646e944881` | [tree-sitter-grammars/tree-sitter-kconfig](https://github.com/tree-sitter-grammars/tree-sitter-kconfig) |
| `arborium-kdl` | 2.18.2 | MIT | `b37e3d58e5c5cf8d739b315d6114e02d42e66664` | [tree-sitter-grammars/tree-sitter-kdl](https://github.com/tree-sitter-grammars/tree-sitter-kdl) |
| `arborium-kotlin` | 2.18.2 | MIT | `57fb4560ba8641865bc0baa6b3f413b236112c4c` | [fwcd/tree-sitter-kotlin](https://github.com/fwcd/tree-sitter-kotlin) |
| `arborium-lean` | 2.18.2 | MIT | `efe6b87145608d12f5996bd7f0cf6095a0e82261` | [Julian/tree-sitter-lean](https://github.com/Julian/tree-sitter-lean) |
| `arborium-lua` | 2.18.2 | MIT | `d76023017f7485eae629cb60d406c7a1ca0f40c9` | [tree-sitter-grammars/tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua) |
| `arborium-make` | 2.18.2 | MIT | `70613f3d812cbabbd7f38d104d60a409c4008b43` | [tree-sitter-grammars/tree-sitter-make](https://github.com/tree-sitter-grammars/tree-sitter-make) |
| `arborium-markdown` | 2.18.2 | MIT | `2dfd57f547f06ca5631a80f601e129d73fc8e9f0` | [tree-sitter-grammars/tree-sitter-markdown](https://github.com/tree-sitter-grammars/tree-sitter-markdown) |
| `arborium-matlab` | 2.18.2 | MIT | Not recorded by package | [acristoffers/tree-sitter-matlab](https://github.com/acristoffers/tree-sitter-matlab) |
| `arborium-meson` | 2.18.2 | MIT | `280b6e59186f18528bab1567f5cc43b78b9cd881` | [tree-sitter-grammars/tree-sitter-meson](https://github.com/tree-sitter-grammars/tree-sitter-meson) |
| `arborium-ninja` | 2.18.2 | MIT | Not recorded by package | [alemuller/tree-sitter-ninja](https://github.com/alemuller/tree-sitter-ninja) |
| `arborium-nix` | 2.18.2 | MIT | `bfcb03e6ddccc5a1eb84cea0176096f85a7af333` | [nix-community/tree-sitter-nix](https://github.com/nix-community/tree-sitter-nix) |
| `arborium-objc` | 2.18.2 | MIT | `181a81b8f23a2d593e7ab4259981f50122909fda` | [tree-sitter-grammars/tree-sitter-objc](https://github.com/tree-sitter-grammars/tree-sitter-objc) |
| `arborium-ocaml` | 2.18.2 | MIT | `3ef7c00b29e41e3a0c1d18e82ea37c64d72b93fc` | [tree-sitter/tree-sitter-ocaml](https://github.com/tree-sitter/tree-sitter-ocaml) |
| `arborium-odin` | 2.18.2 | MIT | `d2ca8efb4487e156a60d5bd6db2598b872629403` | [tree-sitter-grammars/tree-sitter-odin](https://github.com/tree-sitter-grammars/tree-sitter-odin) |
| `arborium-perl` | 2.18.2 | MIT | `ad74e6db234c35d537de9358799a8e0cc4f5dee0` | [tree-sitter-perl/tree-sitter-perl](https://github.com/tree-sitter-perl/tree-sitter-perl) |
| `arborium-php` | 2.18.2 | MIT | `7d07b41ce2d442ca9a90ed85d0075eccc17ae315` | [tree-sitter/tree-sitter-php](https://github.com/tree-sitter/tree-sitter-php) |
| `arborium-postscript` | 2.18.2 | MIT | `33718235e7ce32e355a8dc591b435637a329bc5b` | [smoeding/tree-sitter-postscript](https://github.com/smoeding/tree-sitter-postscript) |
| `arborium-powershell` | 2.18.2 | MIT | `9379c77984af1f3d3d7e3cc5e897de3496725280` | [airbus-cert/tree-sitter-powershell](https://github.com/airbus-cert/tree-sitter-powershell) |
| `arborium-prolog` | 2.18.2 | MIT | `d8d415f6a1cf80ca138524bcc395810b176d40fa` | [https://codeberg.org/foxy/tree-sitter-prolog](https://codeberg.org/foxy/tree-sitter-prolog) |
| `arborium-proto` | 2.18.2 | MIT | `d65a18ce7c2242801f702770114ad08056c7f8c9` | [coder3101/tree-sitter-proto](https://github.com/coder3101/tree-sitter-proto) |
| `arborium-python` | 2.18.2 | MIT | `26855eabccb19c6abf499fbc5b8dc7cc9ab8bc64` | [tree-sitter/tree-sitter-python](https://github.com/tree-sitter/tree-sitter-python) |
| `arborium-query` | 2.18.2 | Apache-2.0 | Not recorded by package | [tree-sitter-grammars/tree-sitter-query](https://github.com/tree-sitter-grammars/tree-sitter-query) |
| `arborium-r` | 2.18.2 | MIT | `0e6ef7741712c09dc3ee6e81c42e919820cc65ef` | [r-lib/tree-sitter-r](https://github.com/r-lib/tree-sitter-r) |
| `arborium-regex` | 2.18.2 | MIT | `b2ac15e27fce703d2f37a79ccd94a5c0cbe9720b` | [tree-sitter/tree-sitter-regex](https://github.com/tree-sitter/tree-sitter-regex) |
| `arborium-rego` | 2.18.2 | MIT | `ddd39af81fe8b0288102a7cb97959dfce723e0f3` | [FallenAngel97/tree-sitter-rego](https://github.com/FallenAngel97/tree-sitter-rego) |
| `arborium-rescript` | 2.18.2 | MIT | `d2df8a285fff95de56a91d2f8152aeceb66f40ef` | [rescript-lang/tree-sitter-rescript](https://github.com/rescript-lang/tree-sitter-rescript) |
| `arborium-ron` | 2.18.2 | MIT OR Apache-2.0 | `78938553b93075e638035f624973083451b29055` | [tree-sitter-grammars/tree-sitter-ron](https://github.com/tree-sitter-grammars/tree-sitter-ron) |
| `arborium-ruby` | 2.18.2 | MIT | Not recorded by package | [tree-sitter/tree-sitter-ruby](https://github.com/tree-sitter/tree-sitter-ruby) |
| `arborium-rust` | 2.18.2 | MIT | `261b20226c04ef601adbdf185a800512a5f66291` | [https://codeberg.org/grammar-orchard/tree-sitter-rust-orchard](https://codeberg.org/grammar-orchard/tree-sitter-rust-orchard) |
| `arborium-scala` | 2.18.2 | MIT | `97aead18d97708190a51d4f551ea9b05b60641c9` | [tree-sitter/tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala) |
| `arborium-scheme` | 2.18.2 | MIT | `591893b9a8b9ec85f5cef86e0cc1028012ad9f0e` | [6cdh/tree-sitter-scheme](https://github.com/6cdh/tree-sitter-scheme) |
| `arborium-scss` | 2.18.2 | MIT | `c478c6868648eff49eb04a4df90d703dc45b312a` | [serenadeai/tree-sitter-scss](https://github.com/serenadeai/tree-sitter-scss) |
| `arborium-solidity` | 2.18.2 | MIT | `4e938a46c7030dd001bc99e1ac0f0c750ac98254` | [JoranHonig/tree-sitter-solidity](https://github.com/JoranHonig/tree-sitter-solidity) |
| `arborium-sparql` | 2.18.2 | MIT | Not recorded by package | [GordianDziwis/tree-sitter-sparql](https://github.com/GordianDziwis/tree-sitter-sparql) |
| `arborium-sql` | 2.18.2 | MIT | `fe77f6868d6cdea593052a6af390116495093dc1` | [DerekStride/tree-sitter-sql](https://github.com/DerekStride/tree-sitter-sql) |
| `arborium-ssh-config` | 2.18.2 | MIT | Not recorded by package | [ObserverOfTime/tree-sitter-ssh-config](https://github.com/ObserverOfTime/tree-sitter-ssh-config) |
| `arborium-starlark` | 2.18.2 | MIT | `a453dbf3ba433db0e5ec621a38a7e59d72e4dc69` | [tree-sitter-grammars/tree-sitter-starlark](https://github.com/tree-sitter-grammars/tree-sitter-starlark) |
| `arborium-styx` | 2.18.2 | MIT OR Apache-2.0 | `643357e9cd940dbc06e7b96e5f1e67fb2f25e8d4` | [bearcove/styx](https://github.com/bearcove/styx) |
| `arborium-svelte` | 2.18.2 | MIT | `ae5199db47757f785e43a14b332118a5474de1a2` | [tree-sitter-grammars/tree-sitter-svelte](https://github.com/tree-sitter-grammars/tree-sitter-svelte) |
| `arborium-swift` | 2.18.2 | MIT | `78d84ef82c387fceeb6094038da28717ea052e39` | [alex-pinkus/tree-sitter-swift](https://github.com/alex-pinkus/tree-sitter-swift) |
| `arborium-textproto` | 2.18.2 | ISC | Not recorded by package | [PorterAtGoogle/tree-sitter-textproto](https://github.com/PorterAtGoogle/tree-sitter-textproto) |
| `arborium-thrift` | 2.18.2 | MIT | `68fd0d80943a828d9e6f49c58a74be1e9ca142cf` | [tree-sitter-grammars/tree-sitter-thrift](https://github.com/tree-sitter-grammars/tree-sitter-thrift) |
| `arborium-tlaplus` | 2.18.2 | MIT | `2d831940c782850f64dabf5b7b17e9e51f7f0ebb` | [tlaplus-community/tree-sitter-tlaplus](https://github.com/tlaplus-community/tree-sitter-tlaplus) |
| `arborium-toml` | 2.18.2 | MIT | `64b56832c2cffe41758f28e05c756a3a98d16f41` | [tree-sitter-grammars/tree-sitter-toml](https://github.com/tree-sitter-grammars/tree-sitter-toml) |
| `arborium-tsx` | 2.18.2 | MIT | `75b3874edb2dc714fb1fd77a32013d0f8699989f` | [tree-sitter/tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript) |
| `arborium-typescript` | 2.18.2 | MIT | `75b3874edb2dc714fb1fd77a32013d0f8699989f` | [tree-sitter/tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript) |
| `arborium-typst` | 2.18.2 | MIT | Not recorded by package | [uben0/tree-sitter-typst](https://github.com/uben0/tree-sitter-typst) |
| `arborium-vb` | 2.18.2 | MIT | `cfca210ce8fdcb5245bd9cd5c47ce0a21a8488d5` | [CodeAnt-AI/tree-sitter-vb-dotnet](https://github.com/CodeAnt-AI/tree-sitter-vb-dotnet) |
| `arborium-verilog` | 2.18.2 | MIT | `227d277b6a1a5e2bf818d6206935722a7503de08` | [tree-sitter/tree-sitter-verilog](https://github.com/tree-sitter/tree-sitter-verilog) |
| `arborium-vhdl` | 2.18.2 | MIT | `a3b2d84990527c7f8f4ae219c332c00c33d2d8e5` | [alemuller/tree-sitter-vhdl](https://github.com/alemuller/tree-sitter-vhdl) |
| `arborium-vim` | 2.18.2 | MIT | `e8e3d62b2dda6e3a56bc7580628e6e1071a7887a` | [tree-sitter-grammars/tree-sitter-vim](https://github.com/tree-sitter-grammars/tree-sitter-vim) |
| `arborium-vue` | 2.18.2 | MIT | `22bdfa6c9fc0f5ffa44c6e938ec46869ac8a99ff` | [tree-sitter-grammars/tree-sitter-vue](https://github.com/tree-sitter-grammars/tree-sitter-vue) |
| `arborium-wit` | 2.18.2 | Apache-2.0 WITH LLVM-exception | `a80c1f47baa5bfb13e2b5f49aa5304e3dab94948` | [bytecodealliance/tree-sitter-wit](https://github.com/bytecodealliance/tree-sitter-wit) |
| `arborium-x86asm` | 2.18.2 | MIT | `n/a` | Local grammar (bundled in Arborium) |
| `arborium-xml` | 2.18.2 | MIT | `5000ae8f22d11fbe93939b05c1e37cf21117162d` | [tree-sitter-grammars/tree-sitter-xml](https://github.com/tree-sitter-grammars/tree-sitter-xml) |
| `arborium-yaml` | 2.18.2 | MIT | `7708026449bed86239b1cd5bce6e3c34dbca6415` | [tree-sitter-grammars/tree-sitter-yaml](https://github.com/tree-sitter-grammars/tree-sitter-yaml) |
| `arborium-yuri` | 2.18.2 | Apache-2.0 | `c6cd96f42bfea62fdb034f50e7805b43e58b0b85` | [https://git.gay/yuri/tree-sitter-yuri](https://git.gay/yuri/tree-sitter-yuri) |
| `arborium-zig` | 2.18.2 | MIT | `6479aa13f32f701c383083d8b28360ebd682fb7d` | [tree-sitter-grammars/tree-sitter-zig](https://github.com/tree-sitter-grammars/tree-sitter-zig) |
| `arborium-zsh` | 2.18.2 | MIT | `c50ded6979eb7aae8b8da5da9ac2e9e984804881` | [georgeharker/tree-sitter-zsh](https://github.com/georgeharker/tree-sitter-zsh) |

The isolated performance harness also uses syntect 5.3.0, two-face 0.5.2+bat-0.26.1, Oniguruma and Lumis 0.19.0 as historical/comparison dependencies. They are not production dependencies. Existing two-face syntax notices remain for that baseline.
