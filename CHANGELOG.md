# Changelog

## [Unreleased]

## [0.2.0](https://github.com/BartoszCiesla/crawk/compare/v0.1.0...v0.2.0)

### ⛰️ Features


- *(discover)* Add workspace root detection and rejection - ([886f6e1](https://github.com/BartoszCiesla/crawk/commit/886f6e19daa5d782d99686504702014ebd79acda))
- *(parser)* Add file size limit to prevent DoS attacks - ([f793d2d](https://github.com/BartoszCiesla/crawk/commit/f793d2d204aa64ad45a2ef4ffdd5059aa92c59d0))
- *(resolve)* Add pub(in path) visibility support in glob expansion - ([af4264e](https://github.com/BartoszCiesla/crawk/commit/af4264e0fb58ed57fcb372f004068559b091f507))
- *(resolve)* Support restricted visibility in glob imports - ([e337b12](https://github.com/BartoszCiesla/crawk/commit/e337b122d8cf9f935fe28796a3427568542cc3db))

### 🐛 Bug Fixes


- *(cli)* Add input validation for module_path argument - ([4b946bc](https://github.com/BartoszCiesla/crawk/commit/4b946bc44f8c2c78cda38bd72332ef7f4210b7c3))
- *(discover)* Add path traversal protection to module resolution - ([08ec2ff](https://github.com/BartoszCiesla/crawk/commit/08ec2ff9589efd6e5ff0544d6a26f2fe9980faba))
- *(error)* Include module path in parse error messages - ([6572458](https://github.com/BartoszCiesla/crawk/commit/65724582ee6f8fcd35b5c67733d5cf3fd2b6ba25))
- *(module)* Correctly resolve inline module dependencies - ([37b95e3](https://github.com/BartoszCiesla/crawk/commit/37b95e3ca1829f8b4364433db6475e154d895370))

### 🚜 Refactor


- *(analyzer)* Extract parse and collection logic - ([1014b62](https://github.com/BartoszCiesla/crawk/commit/1014b62e35f579287aa8520ddf0762243c4758e4))
- *(analyzer)* Eliminate unnecessary clone in file root map - ([7a9b37d](https://github.com/BartoszCiesla/crawk/commit/7a9b37d27838ac8e2e075f32ec35fa5281ebf6c8))
- *(cache)* Extract ParseCache to dedicated module - ([a1327aa](https://github.com/BartoszCiesla/crawk/commit/a1327aa61dce4f9ebb1231b293c431a3ebd48c6f))
- *(cli)* Split monolithic module into focused submodules - ([dba8adf](https://github.com/BartoszCiesla/crawk/commit/dba8adf285ed8227c65579a511b8eba219f38a9b))
- *(cli)* Replace --grouped flag with --format enum - ([b5e1e77](https://github.com/BartoszCiesla/crawk/commit/b5e1e77c6394050562e9e213efdd93be886d99c0))
- *(cli)* Replace process::exit with Result error propagation - ([20dc31d](https://github.com/BartoszCiesla/crawk/commit/20dc31dca799bf291c4e169c728623257b7eaaee))
- *(discover)* Make CrateInfo fields private - ([5a0ea80](https://github.com/BartoszCiesla/crawk/commit/5a0ea8000ae6e55f28faabcf4c3e777c1ff562c1))
- *(error)* Remove Result re-export from crate top-level - ([710a86d](https://github.com/BartoszCiesla/crawk/commit/710a86d4439bd83c0ce0f73fcc3089c6a826229c))
- *(lib)* Split god module into model, error, and analyzer - ([30a85ca](https://github.com/BartoszCiesla/crawk/commit/30a85caa44b1575c1a3e5e6a1896be7fc384194e))
- *(lib)* Split parser and discover into directory modules - ([87421de](https://github.com/BartoszCiesla/crawk/commit/87421de195c71afe91e591d1e75e194d7dd41602))
- *(lib)* Flatten module structure and rename for clarity - ([dde0068](https://github.com/BartoszCiesla/crawk/commit/dde0068c4a72a9b472e7379ef434c411b7b204ec))
- *(model)* Make AnalysisResult fields private with constructor - ([988c6f8](https://github.com/BartoszCiesla/crawk/commit/988c6f855308f5a101ab61687085e65937cec6d1))
- *(model)* Change module_path() return type to &str - ([cd8d9f0](https://github.com/BartoszCiesla/crawk/commit/cd8d9f07c6720aac9edc8b38c98734d2a281eae5))
- *(parser)* Categorize collected references by syntactic role - ([fdf5aa7](https://github.com/BartoszCiesla/crawk/commit/fdf5aa7b78f2a630178724e3c1fbb65580ff1ddd))
- *(parser)* Move test-only methods to cfg(test) impl block - ([6ddfff0](https://github.com/BartoszCiesla/crawk/commit/6ddfff0ccfadb7409cd94a35b002d603f1c30fb2))
- *(parser)* Remove dead include_tests flag and unused methods - ([e815c59](https://github.com/BartoszCiesla/crawk/commit/e815c590a562a7135595b8845b0fd5e19e3f8ee4))
- *(parser)* Remove FileReferences wrapper and clean up dead code - ([7acb8e3](https://github.com/BartoszCiesla/crawk/commit/7acb8e37556f8253318c8b92ff144ae035025bad))
- *(reference)* Hide Segments type from public API - ([0d25b0c](https://github.com/BartoszCiesla/crawk/commit/0d25b0ce46cd87f537aa42771265957ae04b7e4a))
- *(reference)* Extract transformation logic to consumer modules - ([4cde715](https://github.com/BartoszCiesla/crawk/commit/4cde715da72776674130f31aced2c0421ac25a5b))
- *(reference)* Make TypeReference fields private - ([e5f9ba1](https://github.com/BartoszCiesla/crawk/commit/e5f9ba1724ad771b1fa1688e7136c0d444b8866f))
- *(reference)* Fix Segments Deref antipattern - ([0b8262c](https://github.com/BartoszCiesla/crawk/commit/0b8262c9b712b71478a23925c018ea90e15841a1))
- *(resolve)* Deduplicate item visibility extraction logic - ([92155e9](https://github.com/BartoszCiesla/crawk/commit/92155e93237bdec3ecf93ff2b4c865001141a729))
- *(test)* Consolidate test fixtures under fixtures/ directory - ([f3331e8](https://github.com/BartoszCiesla/crawk/commit/f3331e81fa95f529f1010eb8a3337f0a06d334eb))
- Extract shared test detection logic to utils module - ([8efabb0](https://github.com/BartoszCiesla/crawk/commit/8efabb03f5e7abafbfa3a852bf04a855a875ad3a))
- Add #[non_exhaustive] to public enums - ([77356c7](https://github.com/BartoszCiesla/crawk/commit/77356c769de2b5bf1b8c7c3029f006007e2f9ea0))

### 📚 Documentation


- *(model)* Clarify expand_groups and resolve_globs interaction - ([9ec9363](https://github.com/BartoszCiesla/crawk/commit/9ec93633ca530c283ddc1f89eedba9467c71b9da))
- Enhance public API documentation and specify MSRV - ([278579a](https://github.com/BartoszCiesla/crawk/commit/278579ab29066dda552fdf173737bee2d3813704))
- Add origin story explaining the crawk name - ([f7db429](https://github.com/BartoszCiesla/crawk/commit/f7db42907b07df4a894b3bab0bd187fd0cc9c9c3))
- Add comprehensive README with installation and usage guide - ([56e63f5](https://github.com/BartoszCiesla/crawk/commit/56e63f5f4bfafad53a77ce2bc857fb91fdb9115b))

### ⚡ Performance


- *(parser)* Add parse cache and ParseCache type alias - ([1b3ac22](https://github.com/BartoszCiesla/crawk/commit/1b3ac227546311794e1e2ef215e91d36f05515a5))

### 🧪 Testing


- *(discover)* Add unit tests for module path resolution - ([92ef988](https://github.com/BartoszCiesla/crawk/commit/92ef988c5d8f0be16e7fe8e9fc695b894b2153af))
- *(fixtures)* Add pub(in path) test fixtures with glob expansion - ([bf7b43c](https://github.com/BartoszCiesla/crawk/commit/bf7b43c636b064820ec0b8972c1ebba1b7a36fa2))
- *(use_command)* Add coverage for -e -G flag combination - ([a0bbca1](https://github.com/BartoszCiesla/crawk/commit/a0bbca179baf77ae54061a7c9c050e1e0346ce2c))
- *(utils)* Add unit tests for has_cfg_test function - ([99d1a33](https://github.com/BartoszCiesla/crawk/commit/99d1a3348c5da0d54667a1343f5480121e4b1757))
- Add error path and edge case tests - ([9538974](https://github.com/BartoszCiesla/crawk/commit/9538974f0fae1bdacb1ecd185dfa78f906114c84))
- Add comprehensive integration test suite with fixture crate - ([8cd2854](https://github.com/BartoszCiesla/crawk/commit/8cd2854467c4d58bb4413384b17962daa04d7fa2))

### ⚙️ Miscellaneous Tasks


- *(ci)* Configure typos exception for CHANGELOG - ([170b01f](https://github.com/BartoszCiesla/crawk/commit/170b01ff6ca1de886fb2c438ae8c1faa59b7d037))
- *(test)* Remove clippy unwrap_used allow directives - ([a5042e2](https://github.com/BartoszCiesla/crawk/commit/a5042e236490a655d4587234a6da1b491cf1f09d))
- Update taiki-e/install-action to v2.75.7 - ([ff376c1](https://github.com/BartoszCiesla/crawk/commit/ff376c1c9c98cb2537872b00a73756399e2a1dd9))
- Update prek-action to v2.0.1 - ([5480ada](https://github.com/BartoszCiesla/crawk/commit/5480adafda8ff559f49b8eb0eb47a971cb998819))
- Ignore AI assistant and notes files - ([a111243](https://github.com/BartoszCiesla/crawk/commit/a111243cb64c13135095aa7f27a9b8801a1491fb))
- Add clippy.toml and enforce stricter lints - ([2694361](https://github.com/BartoszCiesla/crawk/commit/2694361ec6805513429156fa715f563511055e28))
- Disable semver check in release-plz config - ([cb39bff](https://github.com/BartoszCiesla/crawk/commit/cb39bff00a735e96c5ba2be585ea5a7a2b9f9403))
- Update GitHub Actions workflow dependencies - ([49eda25](https://github.com/BartoszCiesla/crawk/commit/49eda2583b4374fd989e682b3b547936ef63e877))
- Add release environment protection to workflows - ([e8bb04e](https://github.com/BartoszCiesla/crawk/commit/e8bb04e4bfa3f6e984537e2f905a002b1e2f267c))
- Update zizmor-action to v0.5.2 - ([a9e11e7](https://github.com/BartoszCiesla/crawk/commit/a9e11e7eb118ee9f53ebd6aa4ef1020c7ac8c0ce))
- Ignore fixtures/modules/target directory - ([1de080a](https://github.com/BartoszCiesla/crawk/commit/1de080a58468317f7fd9c67b4098532708014364))
- Add cooldown periods to Dependabot configuration - ([b71bc5e](https://github.com/BartoszCiesla/crawk/commit/b71bc5e403651eb4371322df7a6f20e509db32eb))
- Add Dependabot configuration for dependency updates - ([adb16b8](https://github.com/BartoszCiesla/crawk/commit/adb16b8664e658799fb47df063fa89fb319d6424))


## [0.1.0]

### ⛰️ Features


- *(cli)* Add --resolve-globs option to use command - ([bbc0788](https://github.com/BartoszCiesla/crawk/commit/bbc078897e091995bab828869b0bd5cc4f5e0bbd))
- *(cli)* Implement depth truncation for output - ([57fd211](https://github.com/BartoszCiesla/crawk/commit/57fd21125d5bb7ab6572c9a71e6ed69611d62b3d))
- *(cli)* Add --grouped flag for module-organized output - ([2d69ba8](https://github.com/BartoszCiesla/crawk/commit/2d69ba828c94d926350efff02e81606408df6131))
- *(cli)* Add recursive submodule analysis option - ([615b49c](https://github.com/BartoszCiesla/crawk/commit/615b49cf74b955276a8a50da4380438267b46ce6))
- *(cli)* Add file logging support with --log-file option - ([a66e845](https://github.com/BartoszCiesla/crawk/commit/a66e84540240e617291032ab05a5c5bdecd3f4b7))
- *(cli)* Add multi-level verbosity support - ([7feb6ae](https://github.com/BartoszCiesla/crawk/commit/7feb6ae169388516597a97160653f40f960e266e))
- *(cli)* Add depth limiting and improve module resolution - ([b58be33](https://github.com/BartoszCiesla/crawk/commit/b58be33fd6c61c723efc42f6b1d8931d14602c83))
- *(cli)* Add --expand flag to ungroup import statements - ([2bf5796](https://github.com/BartoszCiesla/crawk/commit/2bf5796f8c5bff8a1d96e650241dfd664ffd5622))
- *(cli)* Support nested module paths with :: syntax - ([73e9030](https://github.com/BartoszCiesla/crawk/commit/73e903086772899d64e80569cd3fe79d3c317f31))
- *(cli)* Add argument parsing and expand use statements - ([f6ec0d0](https://github.com/BartoszCiesla/crawk/commit/f6ec0d0830dc834713320af0f2a95dd9b1635d0d))
- *(cli)* Implement basic module dependency analyzer - ([088bf5b](https://github.com/BartoszCiesla/crawk/commit/088bf5b1fadd5caff39333372f3ddcb4e46bb2d4))
- *(lib)* Use structured TypeReference in dependencies - ([756ba9b](https://github.com/BartoszCiesla/crawk/commit/756ba9b523ab27f8f54e5369da412970ddeeb5d7))
- *(module)* Add glob import resolution with inline module support - ([8a982e9](https://github.com/BartoszCiesla/crawk/commit/8a982e96a904445f8291da57867d403ffc7ac9e5))
- *(module)* Resolve relative paths to absolute paths - ([03aa79b](https://github.com/BartoszCiesla/crawk/commit/03aa79bdf9c6b10f56d5759ad3c647e9604804cb))
- *(module)* Add inline module resolution support - ([e4c693d](https://github.com/BartoszCiesla/crawk/commit/e4c693d0d8dc1c981bfbd60453e348b2015d57dc))
- *(module)* Add comprehensive path reference tracking - ([a52e40b](https://github.com/BartoszCiesla/crawk/commit/a52e40b2483869d727b8a15ef8c7766d9f0b64d5))
- *(module)* Add group expansion for import paths - ([7154b8a](https://github.com/BartoszCiesla/crawk/commit/7154b8a01ff893f086b09164d96139a2143c1715))
- *(module)* Add logging and return type references from parse_file - ([2d3db09](https://github.com/BartoszCiesla/crawk/commit/2d3db09e736c8d6dc31d6b50c8f807cdeda1077a))
- *(module)* Add use statement parser and type references - ([79c825f](https://github.com/BartoszCiesla/crawk/commit/79c825f3c3260d417008742376057677f8599d9d))
- *(module)* Add module discovery with cargo metadata - ([1895c09](https://github.com/BartoszCiesla/crawk/commit/1895c09bb73aad6ceeea7f8d579a2fae79a4c01c))
- *(visitor)* Capture all internal crate path references - ([1f66803](https://github.com/BartoszCiesla/crawk/commit/1f66803a66b795d1e643f1414d9e44139fc7ab62))
- Initialize Rust project with hello world - ([dbb34e3](https://github.com/BartoszCiesla/crawk/commit/dbb34e3ee05273dfd8f17fb0a5fd8894cefc65a4))

### 🐛 Bug Fixes


- *(ci)* Correct pre-commit hook type filters for cargo hooks - ([19c409c](https://github.com/BartoszCiesla/crawk/commit/19c409c0dffc36df134b757fc0726b7051a1f016))
- *(module)* Strip binary target prefix from module paths - ([0d5ceee](https://github.com/BartoszCiesla/crawk/commit/0d5ceee814e3c8706edbc153d9e144fd2207df77))
- *(module)* Resolve main binary target modules for glob expansion - ([ed72afc](https://github.com/BartoszCiesla/crawk/commit/ed72afcbf2b437f1702836538bcda24b0b58b702))
- *(module)* Normalize lib alias to crate name in module paths - ([bf5f6d3](https://github.com/BartoszCiesla/crawk/commit/bf5f6d3c25297955b960f30c5c61859ef1f7f0df))
- *(module)* Correct path formatting for nested use statements - ([0f8948d](https://github.com/BartoszCiesla/crawk/commit/0f8948d6fbb02d9bb5473c0b7db5eb9fe0df9000))
- *(use)* Display crate root module name in grouped output - ([5e967ed](https://github.com/BartoszCiesla/crawk/commit/5e967ed264237287588af0ecd8b2a7fa3a990627))
- *(visitor)* Exclude bare self from internal path detection - ([addec05](https://github.com/BartoszCiesla/crawk/commit/addec05c8162061c885a89aa1839e497a272b1f7))

### 🚜 Refactor


- *(cli)* Extract logging configuration into dedicated module - ([fba21a5](https://github.com/BartoszCiesla/crawk/commit/fba21a5b0c800f474793ac4a5ea9abbb2d9382a7))
- *(cli)* Promote path and verbose to global options - ([1898269](https://github.com/BartoszCiesla/crawk/commit/189826913637c9c4a4e6341aff79be1bd24d8763))
- *(cli)* Remove cargo subcommand support - ([83f56a6](https://github.com/BartoszCiesla/crawk/commit/83f56a6d6bb30d0bc9407b832ba15d66f0d525c6))
- *(lib)* Replace manual collection with AST-based analyzer - ([5b446d1](https://github.com/BartoszCiesla/crawk/commit/5b446d14dbfb94bf9268c1f6abd988daa1b7b6c4))
- *(lib)* Reorganize collector and visitor under analysis directory - ([576e7cc](https://github.com/BartoszCiesla/crawk/commit/576e7cc052a2c0d7ad93f58ec0b3dc2eef5a9e2c))
- *(lib)* Reorganize module utilities under module directory - ([58d6a62](https://github.com/BartoszCiesla/crawk/commit/58d6a62855a465a7cef862c2b18cce73c3ef5788))
- *(lib)* Redesign library API with clean separation from CLI - ([822750c](https://github.com/BartoszCiesla/crawk/commit/822750c019c152a644f3afa4c51e9221c4a57d7e))
- *(logging)* Migrate from eprintln to tracing framework - ([52f9ac9](https://github.com/BartoszCiesla/crawk/commit/52f9ac9f91c343a10913cb0ee5fc774768ebaded))
- *(module)* Match binary targets by file stem - ([38c9c01](https://github.com/BartoszCiesla/crawk/commit/38c9c01974a43870e8ba536e26aca24015f4308d))
- *(module)* Remove deprecated analysis module - ([19c4b1b](https://github.com/BartoszCiesla/crawk/commit/19c4b1bfeca8cc2cc7452a14ef409da74e24f684))
- *(module)* Consolidate path suffix into enum type - ([02aa567](https://github.com/BartoszCiesla/crawk/commit/02aa56784d6f5f544dc7a7b75353a39d9f4172c1))
- *(test)* Migrate to insta snapshot testing - ([c230dec](https://github.com/BartoszCiesla/crawk/commit/c230dece5d5a758780d07f7bd01e1dee2b715f46))
- Organize imports and simplify qualified paths - ([d980989](https://github.com/BartoszCiesla/crawk/commit/d980989f0dd6507bca31bb4660d02346892e9244))
- Extract hardcoded strings into named constants - ([37c67d8](https://github.com/BartoszCiesla/crawk/commit/37c67d85d4fe30ac0fab3f3dccddf87e1aac7ff7))
- Apply clippy suggestions for code quality - ([84e7036](https://github.com/BartoszCiesla/crawk/commit/84e70365b77109cf1c443c49debd147f3b447a7c))
- Apply clippy lints and improve code quality - ([4e3210b](https://github.com/BartoszCiesla/crawk/commit/4e3210be06097d118f2146833cebea0716dce941))
- Restructure codebase into modular library - ([04269ce](https://github.com/BartoszCiesla/crawk/commit/04269ce386dc0035bc0acc29373ffcf6957ecc60))

### 📚 Documentation


- *(cli)* Tighten help text for use subcommand - ([9ab9cbf](https://github.com/BartoszCiesla/crawk/commit/9ab9cbf7a728f0d1ee8f3f901601f3a672347ce9))
- *(module)* Remove ignored example documentation - ([790c784](https://github.com/BartoszCiesla/crawk/commit/790c784331e5c627d43ee8e2e5675b27ea125c43))

### 🎨 Styling


- *(lib)* Use imported types instead of qualified paths - ([831038a](https://github.com/BartoszCiesla/crawk/commit/831038a0946044c4e1b36443bb44b247ca15275f))
- Sort dependencies alphabetically in Cargo.toml - ([608d9b4](https://github.com/BartoszCiesla/crawk/commit/608d9b4727b00c7ea3d8c0c05c4231d1dc735b9e))
- Apply cargo fmt formatting - ([0743fb6](https://github.com/BartoszCiesla/crawk/commit/0743fb6d482ba1f0682c0eb4861581c29d5f3aba))

### 🧪 Testing


- *(cli)* Add snapshot tests for command help output - ([3003328](https://github.com/BartoszCiesla/crawk/commit/3003328bced25217e21b3f9479a57f806485b585))
- *(use)* Add comprehensive test coverage for crate name alias - ([15390e3](https://github.com/BartoszCiesla/crawk/commit/15390e30cfd47821511a2e991de14a658860059b))
- *(use)* Add comprehensive snapshot tests for module analysis flags - ([d3d5c05](https://github.com/BartoszCiesla/crawk/commit/d3d5c05587f9c7b8aedb8dbf43c61e2e20c9cd55))
- Add integration test framework for CLI commands - ([804bff2](https://github.com/BartoszCiesla/crawk/commit/804bff21ef3cd325525203b8e1420b3f54e02036))
- Add comprehensive unit tests for core modules - ([119e566](https://github.com/BartoszCiesla/crawk/commit/119e5668f278b3cce57e8614859df4b0dcb7fb1a))

### ⚙️ Miscellaneous Tasks


- *(test)* Add insta snapshot testing support - ([e6e264f](https://github.com/BartoszCiesla/crawk/commit/e6e264fe6b273266100bfb5dd609bfeaeb18cf8d))
- Initialize changelog for release automation - ([96ae7d7](https://github.com/BartoszCiesla/crawk/commit/96ae7d716f3209d314e9a823406201798f355a3b))
- Add GitHub Actions workflows and release automation - ([d78defa](https://github.com/BartoszCiesla/crawk/commit/d78defa7d0af6d4919e47ba97e07c0d1db7f2631))
- Add IDE directories and log file to gitignore - ([953ae97](https://github.com/BartoszCiesla/crawk/commit/953ae976cc48b4b37ef1379aaf3cdbe1178dd3d2))
- Exclude snapshot files from trailing-whitespace check - ([5927b05](https://github.com/BartoszCiesla/crawk/commit/5927b056c73e33e48f63bf321eb891316b726548))
- Add pre-commit configuration with comprehensive hooks - ([39ff7c5](https://github.com/BartoszCiesla/crawk/commit/39ff7c51dec0c578f165f0ff9f7423e84f895ae7))
- Ignore IDE configuration directories - ([e9320d2](https://github.com/BartoszCiesla/crawk/commit/e9320d2f1f363b16987c7afadcf72262c95bd923))

### Build


- *(test)* Configure integration test name to fix snapshot file naming - ([6af48ee](https://github.com/BartoszCiesla/crawk/commit/6af48eeb8d432a917a8715b7480ebaa2634eefab))
- Make build script optional with static-build-config feature - ([497219d](https://github.com/BartoszCiesla/crawk/commit/497219d565811f5132fd7ce7cf7ca292be73effb))
- Add version metadata and build-time constants - ([e9cfb0a](https://github.com/BartoszCiesla/crawk/commit/e9cfb0a0b095290fab718734d54df47bdfe4b6a1))
- Add justfile for development commands - ([c787514](https://github.com/BartoszCiesla/crawk/commit/c787514ae56f86b44052d2187837256d0c828477))
- Pin Rust toolchain to version 1.93.0 - ([afa9796](https://github.com/BartoszCiesla/crawk/commit/afa97961db5826e6927b3b74c3b1a5bb0a1b5f43))
