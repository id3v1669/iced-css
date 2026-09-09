use iced_css_core::resources::{
    age::{AgeField, DeltaSeconds},
    cache_restrictions::MaxAge,
    freshness::*,
};
fn seconds(n: u64) -> DeltaSeconds {
    DeltaSeconds::from_seconds(n)
}
fn input(date: CacheDate, age: AgeField) -> CacheAgeInputs {
    CacheAgeInputs {
        request_time: 1000,
        response_time: 1010,
        now: 1030,
        date,
        age,
    }
}
#[test]
fn age_uses_conservative_maximum_transit_and_residence_without_clock_defaults() {
    let age = AgeField::Seconds {
        value: seconds(30),
        discarded: false,
    };
    let calculated = calculate_cache_age(input(CacheDate::ValidUtcSeconds(990), age)).unwrap();
    assert_eq!(calculated.apparent_age, seconds(20));
    assert_eq!(calculated.corrected_age_value, seconds(40));
    assert_eq!(calculated.corrected_initial_age, seconds(40));
    assert_eq!(calculated.resident_time, seconds(20));
    assert_eq!(calculated.current_age, seconds(60));
    assert_eq!(
        calculate_cache_age(input(CacheDate::ValidUtcSeconds(900), age))
            .unwrap()
            .current_age,
        seconds(130)
    );
    assert_eq!(
        calculate_cache_age(input(CacheDate::Absent, age))
            .unwrap()
            .current_age,
        seconds(60)
    );
    assert_eq!(
        calculate_cache_age(input(CacheDate::ValidUtcSeconds(1100), AgeField::Absent))
            .unwrap()
            .current_age,
        seconds(30)
    );
    assert_eq!(
        calculate_cache_age(input(
            CacheDate::ValidUtcSeconds(1100),
            AgeField::Ignored {
                offset: 0,
                discarded: true
            }
        ))
        .unwrap()
        .current_age,
        seconds(30)
    );
}
#[test]
fn explicit_freshness_obeys_precedence_strict_expiry_and_invalid_evidence() {
    let evaluate = |max_age, expires, date, age| {
        evaluate_explicit_freshness(max_age, expires, date, 1010, seconds(age))
    };
    use CacheDate::*;
    assert_eq!(
        evaluate(MaxAge::Seconds(seconds(60)), Invalid, Invalid, 59).unwrap(),
        ExplicitFreshness::Fresh {
            lifetime: seconds(60),
            remaining: seconds(1)
        }
    );
    assert_eq!(
        evaluate(
            MaxAge::Seconds(seconds(60)),
            ValidUtcSeconds(5000),
            ValidUtcSeconds(990),
            60
        )
        .unwrap(),
        ExplicitFreshness::Stale {
            lifetime: seconds(60),
            overdue: seconds(0)
        }
    );
    assert_eq!(
        evaluate(
            MaxAge::Absent,
            ValidUtcSeconds(1100),
            ValidUtcSeconds(990),
            60
        )
        .unwrap(),
        ExplicitFreshness::Fresh {
            lifetime: seconds(110),
            remaining: seconds(50)
        }
    );
    assert_eq!(
        evaluate(MaxAge::Absent, ValidUtcSeconds(1100), Absent, 60).unwrap(),
        ExplicitFreshness::Fresh {
            lifetime: seconds(90),
            remaining: seconds(30)
        }
    );
    assert_eq!(
        evaluate(
            MaxAge::Absent,
            ValidUtcSeconds(900),
            ValidUtcSeconds(990),
            60
        )
        .unwrap(),
        ExplicitFreshness::Stale {
            lifetime: seconds(0),
            overdue: seconds(60)
        }
    );
    assert_eq!(
        evaluate(
            MaxAge::Invalid {
                first: 0,
                second: Some(1)
            },
            ValidUtcSeconds(5000),
            Absent,
            0
        )
        .unwrap(),
        ExplicitFreshness::InvalidMaxAge
    );
    assert_eq!(
        evaluate(MaxAge::Absent, Invalid, Absent, 0).unwrap(),
        ExplicitFreshness::InvalidExpires
    );
    assert_eq!(
        evaluate(MaxAge::Absent, Absent, Absent, 0).unwrap(),
        ExplicitFreshness::Unspecified
    );
    assert_eq!(
        evaluate(MaxAge::Absent, ValidUtcSeconds(5000), Invalid, 0),
        Err(CacheTimeError::InvalidDate)
    );
}
#[test]
fn invalid_clock_samples_fail_and_extreme_durations_saturate() {
    let initial = input(CacheDate::Absent, AgeField::Absent);
    assert_eq!(
        calculate_cache_age(CacheAgeInputs {
            request_time: 1011,
            ..initial
        }),
        Err(CacheTimeError::ClockOrder)
    );
    assert_eq!(
        calculate_cache_age(CacheAgeInputs {
            now: 1009,
            ..initial
        }),
        Err(CacheTimeError::ClockOrder)
    );
    assert_eq!(
        calculate_cache_age(CacheAgeInputs {
            date: CacheDate::Invalid,
            ..initial
        }),
        Err(CacheTimeError::InvalidDate)
    );
    let extreme = calculate_cache_age(CacheAgeInputs {
        request_time: i64::MIN,
        response_time: i64::MAX,
        now: i64::MAX,
        date: CacheDate::ValidUtcSeconds(i64::MIN),
        age: AgeField::Seconds {
            value: seconds(1),
            discarded: false,
        },
    })
    .unwrap();
    assert_eq!(extreme.apparent_age, seconds(u64::MAX));
    assert_eq!(extreme.corrected_age_value, seconds(u64::MAX));
    assert_eq!(extreme.current_age, seconds(u64::MAX));
    let extreme = calculate_cache_age(CacheAgeInputs {
        request_time: 0,
        response_time: 0,
        now: 1,
        date: CacheDate::Absent,
        age: AgeField::Seconds {
            value: seconds(u64::MAX),
            discarded: false,
        },
    })
    .unwrap();
    assert_eq!(extreme.current_age, seconds(u64::MAX));
    assert_eq!(
        evaluate_explicit_freshness(
            MaxAge::Absent,
            CacheDate::ValidUtcSeconds(i64::MAX),
            CacheDate::ValidUtcSeconds(i64::MIN),
            0,
            seconds(u64::MAX)
        )
        .unwrap(),
        ExplicitFreshness::Stale {
            lifetime: seconds(u64::MAX),
            overdue: seconds(0)
        }
    );
}
