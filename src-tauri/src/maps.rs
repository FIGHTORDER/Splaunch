//! The map list.
//!
//! Inside the lobby this came free - you had been in rooms, so you had seen
//! maps. Standing alone there is no such list, so Splaunch asks Zero-K's own
//! content service for the whole catalogue: one call, every featured and
//! supported map, with the `ResourceID` that addresses its page.
//!
//! `GetPublicCommunityInfo` takes no arguments and answers over plain HTTP -
//! POST over HTTPS 404s on that endpoint, which was measured rather than
//! assumed. The reply is SOAP, so it is read with the same string scanning the
//! lobby uses rather than by taking on an XML dependency for one call.

use serde::Serialize;

const ENDPOINT: &str = "http://zero-k.info/ContentService.svc";
const SOAP_ACTION: &str = "http://tempuri.org/IContentService/GetPublicCommunityInfo";
const ALLOWED_HOST: &str = "zero-k.info";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueMap {
    pub name: String,
    pub resource_id: u32,
    /// Map width and height, in elmos, where the catalogue gives them.
    ///
    /// The editor placed everything against a hardcoded 8x8 before this: click
    /// the middle of a 16x16 map and the unit landed in the top-left quarter of
    /// it. The catalogue carries the size in map units of 512 elmos, so it is
    /// the cheapest place to get a real answer - reading the map's own header
    /// means opening its archive, which is `.sd7` and a separate piece of work.
    ///
    /// `None` when the catalogue does not say, so the caller can fall back
    /// visibly rather than silently adopting a wrong number.
    pub width_elmos: Option<u32>,
    pub height_elmos: Option<u32>,
}

/// One side of a map, in elmos, from the catalogue's map-unit figure.
///
/// Rejected rather than trusted when it is not a plausible map: the field is
/// occasionally absent or zero, and a zero would put every unit at the origin.
fn side_elmos(raw: Option<&str>) -> Option<u32> {
    let units: u32 = raw?.trim().parse().ok()?;
    (1..=64).contains(&units).then_some(units * 512)
}

fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    host == ALLOWED_HOST || host.ends_with(&format!(".{ALLOWED_HOST}"))
}

fn element_text<'a>(xml: &'a str, local: &str) -> Option<&'a str> {
    let open = format!("<a:{local}>");
    let close = format!("</a:{local}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(&xml[start..end])
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Read the `MapItems` out of a `GetPublicCommunityInfo` response.
pub fn parse_catalogue(xml: &str) -> Vec<CatalogueMap> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = xml[at..].find("<a:MapItem>") {
        let start = at + i;
        let Some(end) = xml[start..].find("</a:MapItem>") else { break };
        let item = &xml[start..start + end];
        let name = element_text(item, "Name").map(unescape);
        let id = element_text(item, "ResourceID").and_then(|s| s.trim().parse().ok());
        if let (Some(name), Some(resource_id)) = (name, id) {
            if !name.is_empty() && resource_id != 0 {
                out.push(CatalogueMap {
                    name,
                    resource_id,
                    width_elmos: side_elmos(element_text(item, "Width")),
                    height_elmos: side_elmos(element_text(item, "Height")),
                });
            }
        }
        at = start + end;
    }
    out
}

/// Every map the service knows about.
///
/// Errors are the caller's to show, not to hide: a map picker with no maps in
/// it should say why rather than looking like the catalogue is empty.
#[tauri::command]
pub fn sp_maps() -> Result<Vec<CatalogueMap>, String> {
    if !host_allowed(ENDPOINT) {
        return Err("refusing to fetch from anywhere but zero-k.info".into());
    }
    let body = "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body><GetPublicCommunityInfo xmlns=\"http://tempuri.org/\"/></s:Body></s:Envelope>";

    let res = reqwest::blocking::Client::builder()
        .user_agent(concat!("Splaunch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("could not build an HTTP client: {e}"))?
        .post(ENDPOINT)
        .header("Content-Type", "text/xml; charset=utf-8")
        .header("SOAPAction", format!("\"{SOAP_ACTION}\""))
        .body(body)
        .send()
        .map_err(|e| format!("could not reach the Zero-K content service: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("the content service answered {}", res.status()));
    }
    let text = res.text().map_err(|e| format!("unreadable response: {e}"))?;
    Ok(parse_catalogue(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<a:MapItems><a:MapItem><a:Height>16</a:Height>\
<a:Name>Aberdeen3v3v3</a:Name><a:ResourceID>7116</a:ResourceID></a:MapItem>\
<a:MapItem><a:Name>Comet Catcher Redux</a:Name><a:ResourceID>55646</a:ResourceID>\
</a:MapItem></a:MapItems>";

    #[test]
    fn the_catalogue_is_names_with_the_ids_that_address_them() {
        let maps = parse_catalogue(SAMPLE);
        assert_eq!(maps.len(), 2);
        assert_eq!(maps[0].name, "Aberdeen3v3v3");
        assert_eq!(maps[0].resource_id, 7116);
        assert_eq!(maps[1].name, "Comet Catcher Redux");
    }

    #[test]
    fn a_map_carries_the_size_the_catalogue_knows() {
        // Everything used to be placed against a hardcoded 8x8, so a click in
        // the middle of a 16x16 map landed in the top-left quarter of it.
        let maps = parse_catalogue(SAMPLE);
        assert_eq!(maps[0].height_elmos, Some(16 * 512));
        // The second entry has no dimensions, and says so rather than guessing.
        assert_eq!(maps[1].height_elmos, None);
        assert_eq!(maps[1].width_elmos, None);
    }

    #[test]
    fn an_implausible_size_is_refused() {
        // A zero would put every unit at the origin, which looks like a bug in
        // placement rather than in the catalogue.
        assert_eq!(side_elmos(Some("0")), None);
        assert_eq!(side_elmos(Some("")), None);
        assert_eq!(side_elmos(Some("9999")), None);
        assert_eq!(side_elmos(Some("12")), Some(12 * 512));
    }

    #[test]
    fn a_response_without_maps_is_empty_rather_than_wrong() {
        assert!(parse_catalogue("<s:Envelope/>").is_empty());
    }

    #[test]
    fn an_entry_missing_its_id_is_skipped_rather_than_guessed() {
        let xml = "<a:MapItem><a:Name>Nameless</a:Name></a:MapItem>";
        assert!(parse_catalogue(xml).is_empty());
    }

    #[test]
    fn only_zero_k_is_fetchable() {
        assert!(host_allowed("http://zero-k.info/ContentService.svc"));
        assert!(!host_allowed("http://zero-k.info.example.com/x"));
        assert!(!host_allowed("http://zero-k.info@evil.example/x"));
        assert!(!host_allowed("file:///etc/passwd"));
    }
}
