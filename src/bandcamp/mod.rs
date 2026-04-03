pub mod client;
pub mod models;

#[cfg(test)]
mod tests {
    use super::client::parse_album_page_public;

    fn make_tralbum_html(json: &str) -> String {
        let encoded = json
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\'', "&#39;");
        format!(
            r#"<html><body><div data-tralbum="{}"></div></body></html>"#,
            encoded
        )
    }

    #[test]
    fn parse_album_with_tracks() {
        let json = r#"{"trackinfo":[{"title":"Song One","track_num":1,"duration":200.5,"file":{"mp3-128":"https://example.com/track1.mp3"}},{"title":"Song Two","track_num":2,"duration":180.0,"file":{"mp3-128":"https://example.com/track2.mp3"}}],"current":{"about":"An album about things","credits":"Written by someone","release_date":"01 Jan 2025 00:00:00 GMT"}}"#;
        let html = make_tralbum_html(json);
        let detail = parse_album_page_public(&html).unwrap();

        assert_eq!(detail.tracks.len(), 2);
        assert_eq!(detail.tracks[0].title, "Song One");
        assert_eq!(detail.tracks[0].track_num, 1);
        assert!((detail.tracks[0].duration - 200.5).abs() < 0.01);
        assert_eq!(
            detail.tracks[0].stream_url.as_deref(),
            Some("https://example.com/track1.mp3")
        );
        assert_eq!(detail.about.as_deref(), Some("An album about things"));
        assert_eq!(detail.credits.as_deref(), Some("Written by someone"));
        assert!(detail.release_date.is_some());
    }

    #[test]
    fn parse_album_no_tracks() {
        let json = r#"{"trackinfo":[],"current":null}"#;
        let html = make_tralbum_html(json);
        let detail = parse_album_page_public(&html).unwrap();

        assert!(detail.tracks.is_empty());
        assert!(detail.about.is_none());
    }

    #[test]
    fn parse_missing_tralbum_fails() {
        let html = "<html><body><div>no data here</div></body></html>";
        assert!(parse_album_page_public(html).is_err());
    }

    #[test]
    fn parse_html_entities_in_json() {
        let json = r#"{"trackinfo":[{"title":"Rock & Roll","track_num":1,"duration":120.0,"file":null}],"current":{"about":"It's a <great> album","credits":null,"release_date":null}}"#;
        let html = make_tralbum_html(json);
        let detail = parse_album_page_public(&html).unwrap();

        assert_eq!(detail.tracks[0].title, "Rock & Roll");
        assert_eq!(detail.about.as_deref(), Some("It's a <great> album"));
    }
}
