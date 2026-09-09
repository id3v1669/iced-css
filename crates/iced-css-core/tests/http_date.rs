use iced_css_core::resources::http_date::*;
#[test]
fn imf_date_checks_calendar_weekday_and_explicit_spelling_mode() {
    let parsed = parse_imf_fixdate(b"Sun, 06 Nov 1994 08:49:37 GMT", DateSpelling::Strict).unwrap();
    assert_eq!(
        (
            parsed.year,
            parsed.month,
            parsed.day,
            parsed.hour,
            parsed.minute,
            parsed.second
        ),
        (1994, 11, 6, 8, 49, 37)
    );
    assert_eq!(parsed.weekday, Weekday::Sunday);
    assert!(parse_imf_fixdate(b"sun, 06 nOV 1994 08:49:37 gMt", DateSpelling::Strict).is_err());
    assert_eq!(
        parse_imf_fixdate(
            b"sun, 06 nOV 1994 08:49:37 gMt",
            DateSpelling::CacheRecipient
        )
        .unwrap(),
        parsed
    );
    for invalid in [
        b"Sun, 06 Nov 1994 08:49:37 UTC".as_slice(),
        b"Sun, 06 Nov 1994 08:49:37 GMT ",
        b" Sun, 06 Nov 1994 08:49:37 GMT",
        b"Sun, 06 Nov 1899 08:49:37 GMT",
        b"Sun, 06 Nov 1994 24:49:37 GMT",
        b"Sun, 06 Nov 1994 08:60:37 GMT",
        b"Sun, 06 Nov 1994 08:49:61 GMT",
        b"Thu, 29 Feb 1900 08:49:37 GMT",
        b"Mon, 29 Feb 2100 08:49:37 GMT",
        b"Sun, 31 Apr 2024 08:49:37 GMT",
        b"Sun, 00 Nov 1994 08:49:37 GMT",
        b"Sun, 06 Nov 1994 08:49:37 +00",
        b"Sunday, 06-Nov-94 08:49:37 GMT",
        b"Sun Nov  6 08:49:37 1994",
    ] {
        assert!(
            parse_imf_fixdate(invalid, DateSpelling::CacheRecipient).is_err(),
            "{}",
            String::from_utf8_lossy(invalid)
        );
    }
    assert_eq!(
        parse_imf_fixdate(b"Mon, 06 Nov 1994 08:49:37 GMT", DateSpelling::Strict),
        Err(HttpDateError::Weekday)
    );
    let leap = parse_imf_fixdate(b"Sat, 31 Dec 2016 23:59:60 GMT", DateSpelling::Strict).unwrap();
    assert_eq!(leap.second, 60);
    assert_eq!(
        parse_imf_fixdate(b"Tue, 29 Feb 2000 00:00:00 GMT", DateSpelling::Strict)
            .unwrap()
            .day,
        29
    );
    let valid = b"Sun, 06 Nov 1994 08:49:37 GMT";
    for index in 0..valid.len() {
        let mut invalid = *valid;
        invalid[index] = 0xff;
        assert!(parse_imf_fixdate(&invalid, DateSpelling::CacheRecipient).is_err());
    }
}
#[test]
fn imf_date_matches_retained_independent_python_calendar_corpus() {
    let mut rows = 0;
    for line in include_str!("fixtures/http_imf_calendar.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let parsed = parse_imf_fixdate(fields[0].as_bytes(), DateSpelling::Strict)
            .unwrap_or_else(|e| panic!("{}: {e}", fields[0]));
        assert_eq!(parsed.year, fields[1].parse::<u16>().unwrap());
        assert_eq!(parsed.month, fields[2].parse::<u8>().unwrap());
        assert_eq!(parsed.day, fields[3].parse::<u8>().unwrap());
        rows += 1;
    }
    assert!(rows > 10000);
}

#[test]
fn asctime_accepts_both_day_spellings_and_shares_verified_calendar_semantics() {
    let preferred =
        parse_imf_fixdate(b"Sun, 06 Nov 1994 08:49:37 GMT", DateSpelling::Strict).unwrap();
    assert_eq!(
        parse_asctime_date(b"Sun Nov  6 08:49:37 1994", DateSpelling::Strict).unwrap(),
        preferred
    );
    assert_eq!(
        parse_asctime_date(b"Sun Nov 06 08:49:37 1994", DateSpelling::Strict).unwrap(),
        preferred
    );
    assert_eq!(
        parse_asctime_date(b"sUN nOV  6 08:49:37 1994", DateSpelling::CacheRecipient).unwrap(),
        preferred
    );
    assert!(parse_asctime_date(b"sUN nOV  6 08:49:37 1994", DateSpelling::Strict).is_err());
    for input in [
        b"Sun Nov  6 08:49:37 1994 GMT".as_slice(),
        b"Sun Nov 6 08:49:37 1994",
        b"Sun Nov  0 08:49:37 1994",
        b"Sun Nov 06 08:49:61 1994",
        b"Mon Nov 06 08:49:37 1994",
        b"Thu Feb 29 08:49:37 1900",
        b"Sun Nov 06 08:49:37 1899",
    ] {
        assert!(parse_asctime_date(input, DateSpelling::Strict).is_err());
    }
    assert_eq!(
        parse_asctime_date(b"Sat Dec 31 23:59:60 2016", DateSpelling::Strict)
            .unwrap()
            .second,
        60
    );
    let valid = b"Sun Nov  6 08:49:37 1994";
    for index in 0..valid.len() {
        let mut invalid = *valid;
        invalid[index] = 0xff;
        assert!(parse_asctime_date(&invalid, DateSpelling::CacheRecipient).is_err());
    }
}

#[test]
fn asctime_matches_retained_independent_python_calendar_corpus() {
    let mut rows = 0;
    for line in include_str!("fixtures/http_asctime_calendar.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let parsed = parse_asctime_date(fields[0].as_bytes(), DateSpelling::Strict)
            .unwrap_or_else(|e| panic!("{}: {e}", fields[0]));
        assert_eq!(parsed.year, fields[1].parse::<u16>().unwrap());
        assert_eq!(parsed.month, fields[2].parse::<u8>().unwrap());
        assert_eq!(parsed.day, fields[3].parse::<u8>().unwrap());
        rows += 1;
    }
    assert!(rows > 10000);
}

#[test]
fn generated_http_date_is_canonical_and_rejects_invalid_constructed_fields() {
    let date = HttpDateFields {
        weekday: Weekday::Sunday,
        year: 1994,
        month: 11,
        day: 6,
        hour: 8,
        minute: 49,
        second: 37,
    };
    assert_eq!(
        &format_imf_fixdate(date).unwrap(),
        b"Sun, 06 Nov 1994 08:49:37 GMT"
    );
    for invalid in [
        HttpDateFields { year: 1899, ..date },
        HttpDateFields {
            year: 10000,
            ..date
        },
        HttpDateFields { month: 0, ..date },
        HttpDateFields { month: 13, ..date },
        HttpDateFields { day: 0, ..date },
        HttpDateFields { day: 31, ..date },
        HttpDateFields { hour: 24, ..date },
        HttpDateFields { minute: 60, ..date },
        HttpDateFields { second: 61, ..date },
        HttpDateFields {
            weekday: Weekday::Monday,
            ..date
        },
    ] {
        assert!(format_imf_fixdate(invalid).is_err(), "{invalid:?}");
    }
    let leap = HttpDateFields {
        weekday: Weekday::Saturday,
        year: 2016,
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 60,
    };
    assert_eq!(
        &format_imf_fixdate(leap).unwrap(),
        b"Sat, 31 Dec 2016 23:59:60 GMT"
    );
    for (preferred, obsolete) in include_str!("fixtures/http_imf_calendar.tsv")
        .lines()
        .zip(include_str!("fixtures/http_asctime_calendar.tsv").lines())
    {
        let preferred = preferred.split('\t').next().unwrap();
        let obsolete = obsolete.split('\t').next().unwrap();
        let parsed = parse_asctime_date(obsolete.as_bytes(), DateSpelling::Strict).unwrap();
        assert_eq!(
            format_imf_fixdate(parsed).unwrap().as_slice(),
            preferred.as_bytes()
        );
    }
}
