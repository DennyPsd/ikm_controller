## структура на проект под сборку на кросс

```text
/ikm
├─ ikm-controller/                   основной модуль 
│     └─ …  
├─ ikm-calc/                         модуль расчётов
│     └─ …  
├─ shared-types/                     общие типыструктуры для всех модулей
│     └─ …  
├─ src/                              корневой src текущего workspace-пакета
│     └─ lib.rs                      пустой 
├─ taxon/                            вспомогательный модуль
│     └─ …  
├─ target/                          
│     └─ …  
├─ Cargo.toml                        workspace-манифест 
├─ Cargo.lock                        
└─ Cross.toml                        
```
>   cross build --release --target x86_64-unknown-linux-gnu -p ikm-controller

### содержимое Cargo.toml:
```text
[workspace]
resolver = "3"
members = [
  "ikm-controller",
  "taxon/crates/taxon_core",
  "ikm-calc/ikm_calc",
]

[workspace.package]
authors = ["https://github.com/ni-s-ki"]
version = "0.1.0"
edition = "2024"
rust-version = "1.91"
license = "MIT OR Apache-2.0"
readme = "README.md"
exclude = [
  ".github",
  ".vscode",
  ".idea",
  ".git",
  ".roo",
  "docs/*",
  "assets/*",
  "tmp/*",
  ".gitattributes",
  ".gitignore",
  "pki",
]

[workspace.dependencies]
#------------ storages ------------#
fjall = "2.11"

#------------ utils ------------#
fancy-regex = "0.16"
json-patch = "4.1.0"
thiserror = "^1.0"
anyhow = "^1.0"
csv = "^1.3"
nohash-hasher = { version = "0.2" }
strum = { version = "0.26", default-features = false, features = ["derive"] }
strum_macros = "0.26"
rand = { version = "0.8", default-features = false, features = [
  "serde",
  "alloc",
  "rand_chacha",
] }
convert_case = "0.8"
self_cell = "1.2.1"
nu-ansi-term = "^0.50"
colored_json = "^5"
pbkdf2 = "0.12"
xlsx-handlebars = "0.2.2"

#------------ data types ------------#
chrono = { version = "^0.4", default-features = false, features = [
  "serde",
  "clock",
  "wasmbind",
  "core-error",
  "pure-rust-locales",
] }
url = { version = "^2.5", features = ["serde"] }
uuid = { version = "^1.17", features = ["v4", "v5", "v7", "serde"] }
bytes = "1"
semver = { version = "^1", features = ["serde"] }
smol_str = { version = "0.2", features = ["serde", "arbitrary"] }
smallvec = "1.15.1"
indexmap = { version = "^2.11", features = ["serde"] }
enum-map = { version = "^2.7", features = ["serde"] }
iso8601 = { version = "0.6.3", default-features = false, features = [
  "chrono",
  "serde",
] }
num = { version = "0.2" }

#------------ Parse/Serialize/Validate ------------#
nom = "8"
serde = { version = "^1.0", features = ["derive", "rc"] }
serde-saphyr = "0.0.8"
saphyr = "0.0.6"
serde_json = { version = "^1.0", features = [
  "preserve_order",
  "arbitrary_precision",
  "raw_value",
] }
serde_repr = "0.1"
serde_with = { version = "3.4", features = ["json"] }
schemars = { version = "^1", features = [
  "chrono04",
  "url2",
  "uuid1",
  "smallvec1",
  "smol_str02",
  "semver1",
  "bytes1",
  "indexmap2",
  "raw_value",
  "preserve_order",
] }
validator = { version = "0.19", features = ["derive"] }
toml = "0.9.8"
json-schema-validator-core = "1"

#------------ async ------------#
tokio-util = { version = "^0.7" }
tokio = { version = "^1.44" }
futures = { version = "^0.3" }
ractor = { version = "0.15", default-features = false, features = [
  "async-trait",
  "message_span_propogation",
  "output-port-v2",
] }
futures-channel = "*"
futures-util = "*"

#------------ net ------------#
zeromq = { version = "0.4", default-features = false, features = [
  "tokio-runtime",
  "all-transport",
] }
serialport = { version = "^4", features = ["serde"] }
tokio-serial = { version = "^5" }
axum = { version = "^0.8", features = ["ws"] }
tower = "0.5.2"
tower-http = { version = "0.6.7", features = ["full"] }

#------------ cli ------------#
clap = { version = "^4.5", features = ["derive", "env"] }

#------------ tracing ------------#
tracing = { version = "^0.1", features = ["attributes", "valuable"] }
tracing-subscriber = { version = "^0.3", features = [
  "env-filter",
  "json",
  "local-time",
  "ansi",
  "nu-ansi-term",
] }
tracing-serde = "0.2"
color-eyre = "^0.6"
valuable = { version = "0.1.1", features = ["derive"] }

#------------ tui ------------#
ratatui = { version = "0.29", features = [
  "crossterm",
  "serde",
  "scrolling-regions",
  "all-widgets",
] }
reratui = "0.2"
tui-tree-widget-table = "0.2"
throbber-widgets-tui = "0.9.0"
tui-logger = { version = "0.17.2", features = ["tracing-support"] }
metrics-process = "2.4.2"
```
### содержимого Cross.toml:
```text
[target.x86_64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main"

pre-build = [
  "apt-get update && apt-get install -y pkg-config libudev-dev"
]
```

## структура на чистый икм
```text
/ikm-controller-release                               server:server, 0755
├─ ikm-controller                                     бинарь модуля ikm-controller (x86_64-unknown-linux-gnu)
│                                                     исполняемый файл сервиса
│
├─ config.yaml                                        основной конфиг модуля
│
├─ modbus_settings.yaml                               настройки Modbus (устройства, регистры, опрос)
│
├─ assets/                                            служебные/статические данные модуля                     
│  ├─ db/                                             БД
│  │  ├─ DataChange.yaml                              
│  │  ├─ FacilityEvent.yaml                      
│  │  ├─ FacilityEventRule.yaml                    
│  │  ├─ Product.yaml                           
│  │  ├─ Tank.yaml                            
│  │  └─ calc/                                       
│  │     ├─ <UUID>/                                  
│  │     │  ├─ meta.json                             
│  │     │  └─ time_series.json                      
│  │     └─ …                                       
│  │
│  ├─ report_tempplates/                              
│  │  ├─ kmh_report.xlsx                              Excel-шаблон отчёта (kmh_report)
│  │  └─ …                      
│  │
│  ├─ downloads/                                      директория для выгружаемых файлов
│  │  ├─ 019b0cb5-1ccd-70c1-ac30-f19543c5cb88.xlsx    
│  │  └─ … 
│  │
│  └─ static/                                         фронт-статик
│     └─ …                               
└─ tmp/                                              
   ├─ runtime/
   │  ├─ config.yaml                                 
   │  └─ …                                           
   └─ …                                              
```
>  scp .../ikm-controller-release.zip server@192.168.10.85:~