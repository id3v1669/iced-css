//! Fixed-year HTTP date forms, not a complete HTTP-date or UTC instant adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSpelling {
    Strict,
    /// RFC9111 §4.2 case-insensitive receipt; still requires the GMT zone.
    CacheRecipient,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}
impl Weekday {
    fn number(self) -> u32 {
        match self {
            Self::Sunday => 0,
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
        }
    }
}
/// Calendar fields only. A second of 60 is preserved, without asserting an actual
/// leap-second announcement or choosing how to map it to a linear clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpDateFields {
    pub weekday: Weekday,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpDateError {
    Length,
    Syntax,
    Year,
    Month,
    Day,
    Time,
    Weekday,
}
impl std::fmt::Display for HttpDateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP date fields: {self:?}")
    }
}
impl std::error::Error for HttpDateError {}
fn same(actual: &[u8], expected: &[u8], spelling: DateSpelling) -> bool {
    match spelling {
        DateSpelling::Strict => actual == expected,
        DateSpelling::CacheRecipient => actual.eq_ignore_ascii_case(expected),
    }
}
fn pair(a: u8, b: u8) -> Result<u8, HttpDateError> {
    if !a.is_ascii_digit() || !b.is_ascii_digit() {
        return Err(HttpDateError::Syntax);
    }
    Ok((a - b'0') * 10 + (b - b'0'))
}
/// Parse exactly 29 ASCII bytes using RFC9110 §5.6.7 grammar and the referenced
/// calendar/weekday ranges. Constant bounded work, no allocation, no clock read.
/// Other HTTP-date forms and leading/trailing field whitespace are not accepted.
pub fn parse_imf_fixdate(
    input: &[u8],
    spelling: DateSpelling,
) -> Result<HttpDateFields, HttpDateError> {
    let input: &[u8; 29] = input.try_into().map_err(|_| HttpDateError::Length)?;
    for (index, byte) in [
        (3, b','),
        (4, b' '),
        (7, b' '),
        (11, b' '),
        (16, b' '),
        (19, b':'),
        (22, b':'),
        (25, b' '),
    ] {
        if input[index] != byte {
            return Err(HttpDateError::Syntax);
        }
    }
    if !same(&input[26..29], b"GMT", spelling) {
        return Err(HttpDateError::Syntax);
    }
    let date = HttpDateFields {
        weekday: parse_weekday(&input[..3], spelling)?,
        year: u16::from(pair(input[12], input[13])?) * 100 + u16::from(pair(input[14], input[15])?),
        month: parse_month(&input[8..11], spelling)?,
        day: pair(input[5], input[6])?,
        hour: pair(input[17], input[18])?,
        minute: pair(input[20], input[21])?,
        second: pair(input[23], input[24])?,
    };
    validate(date)
}
fn validate(date: HttpDateFields) -> Result<HttpDateFields, HttpDateError> {
    if !(1900..=9999).contains(&date.year) {
        return Err(HttpDateError::Year);
    }
    if !(1..=12).contains(&date.month) {
        return Err(HttpDateError::Month);
    }
    let year = u32::from(date.year);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match date.month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if date.day == 0 || date.day > days {
        return Err(HttpDateError::Day);
    }
    if date.hour > 23 || date.minute > 59 || date.second > 60 {
        return Err(HttpDateError::Time);
    }
    // RFC3339 Appendix B: March-based month/year. Input has admitted year >=1900
    // and month 1..=12, so subtraction and all sums fit u32 without wrapping.
    let (month, year) = if date.month <= 2 {
        (u32::from(date.month) + 10, year - 1)
    } else {
        (u32::from(date.month) - 2, year)
    };
    let century = year / 100;
    let year = year % 100;
    let weekday =
        ((26 * month - 2) / 10 + u32::from(date.day) + year + year / 4 + century / 4 + 5 * century)
            % 7;
    if weekday != date.weekday.number() {
        return Err(HttpDateError::Weekday);
    }
    Ok(date)
}

fn parse_weekday(input: &[u8], spelling: DateSpelling) -> Result<Weekday, HttpDateError> {
    [
        (b"Sun", Weekday::Sunday),
        (b"Mon", Weekday::Monday),
        (b"Tue", Weekday::Tuesday),
        (b"Wed", Weekday::Wednesday),
        (b"Thu", Weekday::Thursday),
        (b"Fri", Weekday::Friday),
        (b"Sat", Weekday::Saturday),
    ]
    .into_iter()
    .find(|(name, _)| same(input, *name, spelling))
    .map(|(_, day)| day)
    .ok_or(HttpDateError::Syntax)
}
fn parse_month(input: &[u8], spelling: DateSpelling) -> Result<u8, HttpDateError> {
    [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ]
    .into_iter()
    .zip(1u8..=12)
    .find(|(name, _)| same(input, *name, spelling))
    .map(|(_, month)| month)
    .ok_or(HttpDateError::Syntax)
}

/// Parse RFC9110 asctime-date: exactly 24 bytes, with implied UTC and either
/// two digits or space-plus-one-digit for the day. No zone suffix is admitted.
pub fn parse_asctime_date(
    input: &[u8],
    spelling: DateSpelling,
) -> Result<HttpDateFields, HttpDateError> {
    let input: &[u8; 24] = input.try_into().map_err(|_| HttpDateError::Length)?;
    for (index, byte) in [
        (3, b' '),
        (7, b' '),
        (10, b' '),
        (13, b':'),
        (16, b':'),
        (19, b' '),
    ] {
        if input[index] != byte {
            return Err(HttpDateError::Syntax);
        }
    }
    validate(HttpDateFields {
        weekday: parse_weekday(&input[..3], spelling)?,
        year: u16::from(pair(input[20], input[21])?) * 100 + u16::from(pair(input[22], input[23])?),
        month: parse_month(&input[4..7], spelling)?,
        day: pair(if input[8] == b' ' { b'0' } else { input[8] }, input[9])?,
        hour: pair(input[11], input[12])?,
        minute: pair(input[14], input[15])?,
        second: pair(input[17], input[18])?,
    })
}

/// Canonical sender spelling required by RFC9110 §5.6.7. Validates supplied
/// calendar fields before formatting; does not choose a current time or qualify
/// leap-second announcements. No heap allocation is performed.
pub fn format_imf_fixdate(date: HttpDateFields) -> Result<[u8; 29], HttpDateError> {
    let date = validate(date)?;
    let mut output = *b"Sun, 00 Jan 0000 00:00:00 GMT";
    let weekday = match date.weekday {
        Weekday::Sunday => b"Sun",
        Weekday::Monday => b"Mon",
        Weekday::Tuesday => b"Tue",
        Weekday::Wednesday => b"Wed",
        Weekday::Thursday => b"Thu",
        Weekday::Friday => b"Fri",
        Weekday::Saturday => b"Sat",
    };
    output[..3].copy_from_slice(weekday);
    let months = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    output[8..11].copy_from_slice(months[usize::from(date.month - 1)]);
    let century = u8::try_from(date.year / 100).map_err(|_| HttpDateError::Year)?;
    let year = u8::try_from(date.year % 100).map_err(|_| HttpDateError::Year)?;
    // Every value is now at most 99, and each destination is a fixed two-byte span.
    for (offset, value) in [
        (5, date.day),
        (12, century),
        (14, year),
        (17, date.hour),
        (20, date.minute),
        (23, date.second),
    ] {
        output[offset..offset + 2].copy_from_slice(&[b'0' + value / 10, b'0' + value % 10]);
    }
    Ok(output)
}
