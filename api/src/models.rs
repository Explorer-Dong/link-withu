//! Request/response contracts shared by validation, streaming restore and OpenAPI.
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_name() -> String {
    "Link WithU".into()
}
fn default_base() -> String {
    "http://localhost:5201".into()
}
fn default_timezone() -> String {
    "Asia/Shanghai".into()
}
fn default_length() -> i64 {
    6
}
fn unknown() -> String {
    "unknown".into()
}

pub fn tags<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct Tags;
    impl<'de> serde::de::Visitor<'de> for Tags {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 20 string tags")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            while let Some(tag) = seq.next_element::<String>()? {
                if result.len() == 20 {
                    return Err(serde::de::Error::custom("标签过多"));
                }
                result.push(tag);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Tags)
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LinkFields {
    #[schema(min_length = 1, max_length = 2048)]
    pub original_url: String,
    #[serde(default)]
    #[schema(max_length = 120)]
    pub title: String,
    #[serde(default)]
    #[schema(max_length = 2000)]
    pub note: String,
    #[serde(default, deserialize_with = "tags")]
    #[schema(max_items = 20)]
    pub tags: Vec<String>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateLink {
    #[serde(flatten)]
    pub fields: LinkFields,
    #[schema(max_length = 32, pattern = "^[A-Za-z0-9_-]+$")]
    pub short_code: Option<String>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SiteSettings {
    #[serde(default = "default_name")]
    #[schema(min_length = 1, max_length = 60)]
    pub app_name: String,
    #[serde(default = "default_base")]
    #[schema(max_length = 2048)]
    pub base_url: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_length")]
    #[schema(minimum = 4, maximum = 16)]
    pub random_code_length: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct BackupLink {
    #[schema(minimum = 1)]
    pub id: i64,
    #[schema(min_length = 1, max_length = 32)]
    pub short_code: String,
    #[schema(min_length = 1, max_length = 2048)]
    pub original_url: String,
    #[serde(default)]
    #[schema(max_length = 120)]
    pub title: String,
    #[serde(default)]
    #[schema(max_length = 2000)]
    pub note: String,
    #[serde(default, deserialize_with = "tags")]
    #[schema(max_items = 20)]
    pub tags: Vec<String>,
    pub is_custom: bool,
    #[schema(format=DateTime)]
    pub created_at: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct BackupVisit {
    #[schema(minimum = 1)]
    pub id: i64,
    #[schema(minimum = 1)]
    pub short_link_id: i64,
    #[schema(format=DateTime)]
    pub visited_at: String,
    #[serde(default)]
    #[schema(max_length = 45)]
    pub ip: Option<String>,
    #[serde(default)]
    #[schema(max_length = 100)]
    pub country: String,
    #[serde(default)]
    #[schema(max_length = 100)]
    pub region: String,
    #[serde(default)]
    #[schema(max_length = 100)]
    pub city: String,
    #[serde(default)]
    #[schema(max_length = 8192)]
    pub user_agent: Option<String>,
    #[serde(default)]
    #[schema(max_length = 253)]
    pub referrer_domain: Option<String>,
    #[serde(default = "unknown")]
    #[schema(max_length = 20)]
    pub device: String,
    #[serde(default = "unknown")]
    #[schema(max_length = 30)]
    pub browser: String,
    #[serde(default)]
    pub is_bot: bool,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct BackupData {
    #[schema(pattern = "^link-withu-backup$")]
    pub format: String,
    #[schema(minimum = 1, maximum = 1)]
    pub version: i64,
    #[schema(format=DateTime)]
    pub exported_at: String,
    pub settings: SiteSettings,
    pub links: Vec<BackupLink>,
    pub visits: Vec<BackupVisit>,
}
#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(max_length = 1024)]
    pub password: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LinkResponse {
    pub id: i64,
    pub short_code: String,
    pub original_url: String,
    pub title: String,
    pub note: String,
    pub tags: Vec<String>,
    pub short_url: String,
    pub is_custom: bool,
    #[schema(format=DateTime)]
    pub created_at: String,
    pub visit_count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LinkList {
    pub total: i64,
    pub items: Vec<LinkResponse>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Tag {
    pub name: String,
    pub link_count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Health {
    pub status: String,
    pub version: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Site {
    pub app_name: String,
    pub version: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    pub detail: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct RestoreResponse {
    pub links: i64,
    pub visits: i64,
    pub message: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct AdminSettings {
    pub site: SiteSettings,
    pub version: String,
    pub database_bytes: u64,
    pub trusted_proxy_count: usize,
    pub max_restore_bytes: usize,
    pub links: i64,
    pub visits: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct VisitStats {
    pub short_code: String,
    pub short_url: String,
    pub original_url: String,
    pub total_visits: i64,
    pub visits: Vec<VisitDetails>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct VisitDetails {
    #[schema(format=DateTime)]
    pub visited_at: String,
    pub ip: Option<String>,
    pub country: String,
    pub region: String,
    pub city: String,
    /// IP 估算地址，优先展示国家与城市；缺少城市时使用地区。
    pub location: String,
    pub user_agent: Option<String>,
    pub referrer_domain: Option<String>,
    pub device: String,
    pub browser: String,
    pub is_bot: bool,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Analytics {
    pub timezone: String,
    pub days: i64,
    pub bucket_days: i64,
    pub range_start: String,
    pub range_end: String,
    pub exclude_bots: bool,
    pub summary: Summary,
    pub trend: Vec<Trend>,
    pub top_links: Vec<RankedLink>,
    pub referrers: Vec<Count>,
    pub devices: Vec<Count>,
    pub browsers: Vec<Count>,
    pub locations: Vec<Count>,
    pub tags: Vec<TagCount>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Summary {
    pub links: i64,
    pub active_links: i64,
    pub total_visits: i64,
    pub today: i64,
    pub last_7_days: i64,
    pub last_30_days: i64,
    pub period_visits: i64,
    pub suspected_bots: i64,
    pub pending_locations: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Trend {
    pub date: String,
    pub end_date: String,
    pub count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct RankedLink {
    pub short_code: String,
    pub title: String,
    pub original_url: String,
    pub count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Count {
    pub name: String,
    pub count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TagCount {
    pub name: String,
    pub link_count: i64,
    pub count: i64,
}
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum BreakdownItem {
    Link(RankedLink),
    Tag(TagCount),
    Count(Count),
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct BreakdownPage {
    pub total: i64,
    pub items: Vec<BreakdownItem>,
}
