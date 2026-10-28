//! Build the API document from the same Rust contracts used by the handlers.
use crate::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::health,
        api::login,
        api::site,
        api::read_settings,
        api::update_settings,
        api::generate_api_key,
        api::revoke_api_key,
        api::create,
        api::list,
        api::tags,
        api::get_link,
        api::visits,
        api::update,
        api::delete,
        api::redirect,
        api::backup,
        analytics::analytics,
        analytics::breakdown,
        analytics::export,
        restore::receive
    ),
    components(schemas(models::ApiError))
)]
struct ApiDoc;
pub fn document(c: &Config) -> Value {
    document_for(&c.app_name)
}
pub fn document_for(app_name: &str) -> Value {
    let mut doc = ApiDoc::openapi();
    doc.info.title = app_name.into();
    doc.info.version = VERSION.into();
    doc.info.description = Some("在设置页面生成 API Key，使用 Authorization: Bearer <API Key> 访问管理接口。API Key 拥有管理员权限，仅在生成时显示；重新生成、撤销或修改管理员密码后失效。短链管理、本地访问统计与在线 IP 地理位置解析的结果保存在本地。".into());
    let mut doc = serde_json::to_value(doc).expect("OpenAPI serialization");
    doc["components"]["securitySchemes"] = json!({"bearer_auth":{"type":"http","scheme":"bearer","description":"在设置页面生成 API Key，放入 Authorization: Bearer <API Key>。"}});
    doc
}
pub fn allowed_methods(path: &str) -> Option<String> {
    // Only used on unmatched API methods; keep routing and documentation aligned.
    let routes = ApiDoc::openapi();
    let doc = serde_json::to_value(routes).ok()?;
    let mut allowed = Vec::new();
    for (pattern, operations) in doc["paths"].as_object()? {
        let segments: Vec<_> = pattern.split('/').collect();
        let actual: Vec<_> = path.split('/').collect();
        if segments.len() != actual.len()
            || !segments
                .iter()
                .zip(actual)
                .all(|(a, b)| *a == b || (a.starts_with('{') && !b.is_empty()))
        {
            continue;
        }
        for method in operations.as_object()?.keys() {
            if ["get", "post", "put", "delete", "patch", "head", "options"]
                .contains(&method.as_str())
            {
                allowed.push(method.to_ascii_uppercase());
            }
        }
    }
    allowed.sort();
    allowed.dedup();
    if allowed.is_empty() {
        None
    } else {
        Some(allowed.join(", "))
    }
}
