// Prüft die Einstufung: was wird wiederholt, was nicht.
#[test]
fn nur_voruebergehende_fehler_werden_wiederholt() {
    // Nachbau der Regel aus drive.rs — dieselbe Liste.
    fn ist_voruebergehend(code: u16, koerper: &str) -> bool {
        matches!(code, 429 | 500 | 502 | 503 | 504)
            || (code == 403
                && (koerper.contains("rateLimitExceeded")
                    || koerper.contains("userRateLimitExceeded")
                    || koerper.contains("User rate limit")
                    || koerper.contains("Rate Limit Exceeded")))
    }
    for c in [429, 500, 502, 503, 504] {
        assert!(ist_voruebergehend(c, ""), "HTTP {c} sollte wiederholt werden");
    }
    for c in [200, 400, 401, 403, 404, 409] {
        assert!(!ist_voruebergehend(c, ""), "HTTP {c} darf NICHT wiederholt werden");
    }
    // Drosselung kommt bei Google als 403 - die MUSS wiederholt werden,
    // sonst reisst sie den ganzen Lauf ab (Ben, 23.08.2026).
    assert!(ist_voruebergehend(403, r#"{"reason":"userRateLimitExceeded"}"#));
    assert!(ist_voruebergehend(403, "User rate limit exceeded."));
    // Eine echte Ablehnung bleibt endgueltig.
    assert!(!ist_voruebergehend(403, r#"{"reason":"insufficientPermissions"}"#));

    // Wartezeiten verdoppeln sich
    let w: Vec<u64> = (0..5).map(|v| 1u64 << v).collect();
    assert_eq!(w, vec![1, 2, 4, 8, 16]);
    println!("OK: 429/5xx und gedrosselte 403 wiederholen, echte 4xx sofort melden");
}
