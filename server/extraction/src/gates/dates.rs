use std::collections::HashMap;
use std::sync::LazyLock;

use pure_rust_locales::{locale_match, Locale};
use regex::Regex;
use time::macros::date;
use time::{Date, Duration, Month, Time};

use super::amounts::normalize_digits;

const MIN_MONTH_NAME_CHARS: usize = 3;
const BUDDHIST_ERA_OFFSET: u32 = 543;
const SERIAL_DATE_EPOCH: Date = date!(1899 - 12 - 30);
const ERAS: [(&str, u32); 12] = [
    ("令和", 2018),
    ("平成", 1988),
    ("昭和", 1925),
    ("大正", 1911),
    ("明治", 1867),
    ("民國", 1911),
    ("民国", 1911),
    ("R", 2018),
    ("H", 1988),
    ("S", 1925),
    ("T", 1911),
    ("M", 1867),
];

static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+").expect("valid regex"));
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+").expect("valid regex"));
static TIME_OF_DAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\b|T)([0-9]{1,2}):([0-9]{2})(?::[0-9]{2}(?:\.[0-9]+)?)?(?:Z|\b)")
        .expect("valid regex")
});
static ANY_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:\b|T)([0-9]{1,2})(?::([0-9]{2})(?::([0-9]{2}))?|\.([0-9]{2})(?:\.([0-9]{2}))?|h([0-9]{2}))(?:\s?([AaPp])\.?[Mm]\.?\b|Z|\b)",
    )
    .expect("valid regex")
});
static PACKED_DATE_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b((?:19|20)[0-9]{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12][0-9]|3[01]))T?([01][0-9]|2[0-3])([0-5][0-9])([0-5][0-9])?(?:\.[0-9]+)?(?:\[[^\]]*\])?([^0-9]|$)",
    )
    .expect("valid regex")
});
static DIGITS_ONLY_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|[^0-9])(?:[0-9]{1,4}[-./][0-9]{1,2}[-./][0-9]{1,4}|[0-9]{1,2}[-/][0-9]{1,2}|[0-9]{1,2}\.[0-9]{1,2}\.|[0-9]{8})(?:[^0-9]|$)",
    )
    .expect("valid regex")
});
static SERIAL_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]{5})(?:\.[0-9]+)?$").expect("valid regex"));
static NUMERIC_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|[^0-9])(?:[0-9]{1,4}[-./][0-9]{1,2}[-./][0-9]{1,4}|[0-9]{1,2}/[0-9]{1,2}|(?:19|20)[0-9]{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12][0-9]|3[01]))(?:[^0-9]|$)|[0-9]{1,4}\s*[年년][\s)）(（0-9年]*[0-9]{1,2}\s*[月월]|[0-9]{1,2}\s*[月월]\s*[0-9]{1,2}\s*[日일]",
    )
    .expect("valid regex")
});
static DAY_AND_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([0-9]{1,2})[\s.,/-]{0,3}(\p{L}{3,})|(\p{L}{3,})[\s.,/-]{0,3}([0-9]{1,2})\b")
        .expect("valid regex")
});

static MONTH_NAMES: LazyLock<HashMap<String, u16>> = LazyLock::new(|| {
    use Locale::*;
    let mut names: HashMap<String, u16> = HashMap::new();
    for locale in [
        aa_DJ,
        aa_ER,
        aa_ER_saaho,
        aa_ET,
        af_ZA,
        agr_PE,
        ak_GH,
        am_ET,
        an_ES,
        anp_IN,
        ar_AE,
        ar_BH,
        ar_DZ,
        ar_EG,
        ar_IN,
        ar_IQ,
        ar_JO,
        ar_KW,
        ar_LB,
        ar_LY,
        ar_MA,
        ar_OM,
        ar_QA,
        ar_SA,
        ar_SD,
        ar_SS,
        ar_SY,
        ar_TN,
        ar_YE,
        as_IN,
        ast_ES,
        ayc_PE,
        az_AZ,
        az_IR,
        be_BY,
        be_BY_latin,
        bem_ZM,
        ber_DZ,
        ber_MA,
        bg_BG,
        bhb_IN,
        bho_IN,
        bho_NP,
        bi_VU,
        bn_BD,
        bn_IN,
        bo_CN,
        bo_IN,
        br_FR,
        br_FR_euro,
        brx_IN,
        bs_BA,
        byn_ER,
        ca_AD,
        ca_ES,
        ca_ES_euro,
        ca_ES_valencia,
        ca_FR,
        ca_IT,
        ce_RU,
        chr_US,
        cmn_TW,
        crh_UA,
        cs_CZ,
        csb_PL,
        cv_RU,
        cy_GB,
        da_DK,
        de_AT,
        de_AT_euro,
        de_BE,
        de_BE_euro,
        de_CH,
        de_DE,
        de_DE_euro,
        de_IT,
        de_LI,
        de_LU,
        de_LU_euro,
        doi_IN,
        dsb_DE,
        dv_MV,
        dz_BT,
        el_CY,
        el_GR,
        el_GR_euro,
        en_AG,
        en_AU,
        en_BW,
        en_CA,
        en_DK,
        en_GB,
        en_HK,
        en_IE,
        en_IE_euro,
        en_IL,
        en_IN,
        en_NG,
        en_NZ,
        en_PH,
        en_SC,
        en_SG,
        en_US,
        en_ZA,
        en_ZM,
        en_ZW,
        eo,
        es_AR,
        es_BO,
        es_CL,
        es_CO,
        es_CR,
        es_CU,
        es_DO,
        es_EC,
        es_ES,
        es_ES_euro,
        es_GT,
        es_HN,
        es_MX,
        es_NI,
        es_PA,
        es_PE,
        es_PR,
        es_PY,
        es_SV,
        es_US,
        es_UY,
        es_VE,
        et_EE,
        eu_ES,
        eu_ES_euro,
        fa_IR,
        ff_SN,
        fi_FI,
        fi_FI_euro,
        fil_PH,
        fo_FO,
        fr_BE,
        fr_BE_euro,
        fr_CA,
        fr_CH,
        fr_FR,
        fr_FR_euro,
        fr_LU,
        fr_LU_euro,
        fur_IT,
        fy_DE,
        fy_NL,
        ga_IE,
        ga_IE_euro,
        gd_GB,
        gez_ER,
        gez_ER_abegede,
        gez_ET,
        gez_ET_abegede,
        gl_ES,
        gl_ES_euro,
        gu_IN,
        gv_GB,
        ha_NG,
        hak_TW,
        he_IL,
        hi_IN,
        hif_FJ,
        hne_IN,
        hr_HR,
        hsb_DE,
        ht_HT,
        hu_HU,
        hy_AM,
        ia_FR,
        id_ID,
        ig_NG,
        ik_CA,
        is_IS,
        it_CH,
        it_IT,
        it_IT_euro,
        iu_CA,
        ja_JP,
        ka_GE,
        kab_DZ,
        kk_KZ,
        kl_GL,
        km_KH,
        kn_IN,
        ko_KR,
        kok_IN,
        ks_IN,
        ks_IN_devanagari,
        ku_TR,
        kw_GB,
        ky_KG,
        lb_LU,
        lg_UG,
        li_BE,
        li_NL,
        lij_IT,
        ln_CD,
        lo_LA,
        lt_LT,
        lv_LV,
        lzh_TW,
        mag_IN,
        mai_IN,
        mai_NP,
        mfe_MU,
        mg_MG,
        mhr_RU,
        mi_NZ,
        miq_NI,
        mjw_IN,
        mk_MK,
        ml_IN,
        mn_MN,
        mni_IN,
        mnw_MM,
        mr_IN,
        ms_MY,
        mt_MT,
        my_MM,
        nan_TW,
        nan_TW_latin,
        nb_NO,
        nds_DE,
        nds_NL,
        ne_NP,
        nhn_MX,
        niu_NU,
        niu_NZ,
        nl_AW,
        nl_BE,
        nl_BE_euro,
        nl_NL,
        nl_NL_euro,
        nn_NO,
        nr_ZA,
        nso_ZA,
        oc_FR,
        om_ET,
        om_KE,
        or_IN,
        os_RU,
        pa_IN,
        pa_PK,
        pap_AW,
        pap_CW,
        pl_PL,
        ps_AF,
        pt_BR,
        pt_PT,
        pt_PT_euro,
        quz_PE,
        raj_IN,
        ro_RO,
        ru_RU,
        ru_UA,
        rw_RW,
        sa_IN,
        sah_RU,
        sat_IN,
        sc_IT,
        sd_IN,
        sd_IN_devanagari,
        se_NO,
        sgs_LT,
        shn_MM,
        shs_CA,
        si_LK,
        sid_ET,
        sk_SK,
        sl_SI,
        sm_WS,
        so_DJ,
        so_ET,
        so_KE,
        so_SO,
        sq_AL,
        sq_MK,
        sr_ME,
        sr_RS,
        sr_RS_latin,
        ss_ZA,
        st_ZA,
        sv_FI,
        sv_FI_euro,
        sv_SE,
        sw_KE,
        sw_TZ,
        szl_PL,
        ta_IN,
        ta_LK,
        tcy_IN,
        te_IN,
        tg_TJ,
        th_TH,
        the_NP,
        ti_ER,
        ti_ET,
        tig_ER,
        tk_TM,
        tl_PH,
        tn_ZA,
        to_TO,
        tpi_PG,
        tr_CY,
        tr_TR,
        ts_ZA,
        tt_RU,
        tt_RU_iqtelif,
        ug_CN,
        uk_UA,
        unm_US,
        ur_IN,
        ur_PK,
        uz_UZ,
        uz_UZ_cyrillic,
        ve_ZA,
        vi_VN,
        wa_BE,
        wa_BE_euro,
        wae_CH,
        wal_ET,
        wo_SN,
        xh_ZA,
        yi_US,
        yo_NG,
        yue_HK,
        yuw_PG,
        zh_CN,
        zh_HK,
        zh_SG,
        zh_TW,
        zu_ZA,
    ] {
        let full = locale_match!(locale => LC_TIME::MON);
        let short = locale_match!(locale => LC_TIME::ABMON);
        let standalone = locale_match!(locale => LC_TIME::ALT_MON);
        for list in [Some(full), Some(short), standalone].into_iter().flatten() {
            for (index, name) in list.iter().enumerate() {
                let name = name.trim().trim_end_matches('.').to_lowercase();
                if !name.chars().all(char::is_alphabetic) {
                    continue;
                }
                let chars: Vec<char> = name.chars().collect();
                for end in MIN_MONTH_NAME_CHARS..=chars.len() {
                    *names.entry(chars[..end].iter().collect()).or_default() |= 1 << (index + 1);
                }
            }
        }
    }
    names
});

pub fn months_named(word: &str) -> Option<u16> {
    MONTH_NAMES.get(&word.to_lowercase()).copied()
}

pub fn contains_date_shape(text: &str) -> bool {
    let text = normalize_digits(text);
    let month = |number: &u32| (1..=12).contains(number);
    let day = |number: &u32| (1..=31).contains(number);
    let mut from = 0;
    while let Some(shape) = NUMERIC_DATE.find_at(&text, from) {
        let parts: Vec<&str> = shape
            .as_str()
            .trim_matches(|c: char| !c.is_ascii_digit())
            .split(['-', '.', '/'])
            .collect();
        let numbers: Option<Vec<u32>> = parts.iter().map(|part| part.parse().ok()).collect();
        match numbers.as_deref() {
            Some([first, second, third])
                if !(month(second) && (day(first) || day(third)))
                    && !(month(first) && day(second)) => {}
            _ => return true,
        }
        let delimiter = shape.as_str().chars().next_back().map_or(1, char::len_utf8);
        from = (shape.end() - delimiter).max(from + 1);
        while !text.is_char_boundary(from) {
            from += 1;
        }
    }
    PACKED_DATE_TIME.is_match(&text) || has_day_and_month_name(&text)
}

pub fn strip_date_shapes(text: &str) -> String {
    let text = normalize_digits(text);
    let text = NUMERIC_DATE.replace_all(&text, " ");
    let text = TIME_OF_DAY.replace_all(&text, " ");
    DAY_AND_WORD
        .replace_all(&text, |captures: &regex::Captures| {
            let word = captures
                .get(2)
                .or_else(|| captures.get(3))
                .map_or("", |m| m.as_str());
            if months_named(word).is_some() {
                " ".to_string()
            } else {
                captures[0].to_string()
            }
        })
        .into_owned()
}

fn has_day_and_month_name(text: &str) -> bool {
    DAY_AND_WORD.captures_iter(text).any(|captures| {
        let word = captures
            .get(2)
            .or_else(|| captures.get(3))
            .map_or("", |m| m.as_str());
        months_named(word).is_some()
    })
}

pub fn date_matches(date_text: &str, date: Date) -> bool {
    let text = normalize_digits(date_text);
    if let Some(serial) = SERIAL_DATE.captures(text.trim()) {
        return serial[1]
            .parse()
            .ok()
            .and_then(|days| SERIAL_DATE_EPOCH.checked_add(Duration::days(days)))
            == Some(date);
    }
    let text = PACKED_DATE_TIME.replace_all(&text, "$1 $5");
    let text = TIME_OF_DAY.replace_all(&text, " ");
    let numbers: Vec<&str> = NUMBERS.find_iter(&text).map(|m| m.as_str()).collect();
    let words: Vec<&str> = WORDS.find_iter(&text).map(|m| m.as_str()).collect();
    let digits_only = digits_are_the_date(&text);
    let named = if digits_only {
        None
    } else {
        words
            .iter()
            .filter_map(|w| months_named(w))
            .reduce(|a, b| a | b)
    };
    let era = ERAS
        .iter()
        .find(|(marker, _)| words.contains(marker))
        .map(|(_, base)| *base);

    let day = u32::from(date.day());
    let month = u32::from(u8::from(date.month()));
    let year = date.year().unsigned_abs();
    let value = |n: &str| n.parse::<u32>().ok();
    let is_day = |n: &str| n.len() <= 2 && value(n) == Some(day);
    let is_month = |n: &str| n.len() <= 2 && value(n) == Some(month);
    let in_era = |n: &str| era.is_some_and(|base| value(n) == year.checked_sub(base));
    let is_year = |n: &str| match n.len() {
        4 => value(n) == Some(year) || value(n) == Some(year + BUDDHIST_ERA_OFFSET),
        2 => value(n) == Some(year % 100) || in_era(n),
        _ => in_era(n),
    };
    let months: Vec<(std::ops::Range<usize>, u16)> = WORDS
        .find_iter(&text)
        .filter_map(|word| Some((word.range(), months_named(word.as_str())?)))
        .collect();
    let places: Vec<std::ops::Range<usize>> = NUMBERS.find_iter(&text).map(|m| m.range()).collect();
    let in_its_month = |number: usize| {
        let place = &places[number];
        months
            .iter()
            .min_by_key(|(word, _)| {
                word.start
                    .saturating_sub(place.end)
                    .max(place.start.saturating_sub(word.end))
            })
            .is_none_or(|(_, mask)| mask & (1 << month) != 0)
    };
    let day_and_month = |a: &str, b: &str| (is_day(a) && is_month(b)) || (is_month(a) && is_day(b));
    let numeric_date = |three: &[&str]| {
        let [a, b, c] = [three[0], three[1], three[2]];
        if a.len() == 4 {
            is_year(a) && is_month(b) && is_day(c)
        } else if c.len() == 4 {
            is_year(c) && day_and_month(a, b)
        } else {
            (is_year(c) && day_and_month(a, b)) || (is_year(a) && is_month(b) && is_day(c))
        }
    };

    if let Some(mask) = named {
        if mask & (1 << month) == 0 {
            return false;
        }
    }
    if let [compact] = numbers.as_slice() {
        let part = |range: std::ops::Range<usize>| &compact[range];
        let reads = |(y, m, d): (std::ops::Range<usize>, _, _)| {
            is_year(part(y)) && is_month(part(m)) && is_day(part(d))
        };
        match compact.len() {
            8 => {
                return [(0..4, 4..6, 6..8), (4..8, 2..4, 0..2), (4..8, 0..2, 2..4)]
                    .into_iter()
                    .any(reads)
            }
            6 => {
                return [(0..2, 2..4, 4..6), (4..6, 2..4, 0..2), (4..6, 0..2, 2..4)]
                    .into_iter()
                    .any(reads)
            }
            _ => {}
        }
    }

    match (named.is_some(), numbers.as_slice()) {
        (true, [d]) => is_day(d),
        (true, [a, b, c]) => {
            (is_day(a) && in_its_month(0) && is_year(b))
                || (is_year(a)
                    && ((is_day(b) && in_its_month(1)) || (is_day(c) && in_its_month(2))))
                || (is_year(c)
                    && ((is_day(a) && in_its_month(0)) || (is_day(b) && in_its_month(1))))
        }
        (true, [a, b, ..]) => {
            (is_day(a) && in_its_month(0) && is_year(b))
                || (is_year(a) && is_day(b) && in_its_month(1))
        }
        (false, [_, _, _]) => numeric_date(&numbers),
        (false, [a, b, _, _, year]) => {
            numeric_date(&numbers[..3])
                || numeric_date(&numbers[2..])
                || numeric_date(&[*a, *b, *year])
        }
        (false, [_, _, _, _, ..]) => {
            numeric_date(&numbers[..3]) || numeric_date(&numbers[numbers.len() - 3..])
        }
        (false, [a, b]) if words.is_empty() || digits_only => day_and_month(a, b),
        (false, [a, b]) => {
            (is_day(a) && is_year(b)) || (is_year(a) && is_day(b)) || day_and_month(a, b)
        }
        (false, [d]) => !words.is_empty() && is_day(d),
        _ => false,
    }
}

pub fn time_appears(text: &str, time: Time) -> bool {
    let text = normalize_digits(text);
    let number = |captures: &regex::Captures, group| {
        captures
            .get(group)
            .and_then(|m| m.as_str().parse::<u8>().ok())
    };
    let reads = |hour: Option<u8>, minute: Option<u8>, second: Option<u8>| {
        hour == Some(time.hour())
            && minute == Some(time.minute())
            && second.unwrap_or(0) == time.second()
    };
    PACKED_DATE_TIME.captures_iter(&text).any(|captures| {
        reads(
            number(&captures, 2),
            number(&captures, 3),
            number(&captures, 4),
        )
    }) || ANY_TIME.captures_iter(&text).any(|captures| {
        let afternoon = captures
            .get(7)
            .map(|marker| marker.as_str().eq_ignore_ascii_case("p"));
        let hour = match (number(&captures, 1), afternoon) {
            (Some(hour @ 1..=12), Some(afternoon)) => {
                Some(hour % 12 + if afternoon { 12 } else { 0 })
            }
            (hour, _) => hour,
        };
        let minute = [2, 4, 6]
            .into_iter()
            .find_map(|group| number(&captures, group));
        let second = [3, 5]
            .into_iter()
            .find_map(|group| number(&captures, group));
        reads(hour, minute, second)
    })
}

pub fn has_month_name(date_text: &str) -> bool {
    !digits_are_the_date(&normalize_digits(date_text))
        && WORDS
            .find_iter(date_text)
            .any(|word| months_named(word.as_str()).is_some())
}

fn digits_are_the_date(text: &str) -> bool {
    DIGITS_ONLY_DATE.find(text).is_some_and(|shape| {
        NUMBERS.find_iter(shape.as_str()).count() != 2
            || !NUMBERS.find_iter(text).any(|number| number.len() == 4)
    })
}

pub fn prints_full_year(date_text: &str, date: Date) -> bool {
    date_matches(date_text, date) && !reads_years_later(date_text, date, 400)
}

pub fn prints_no_date(date_text: &str) -> bool {
    !normalize_digits(date_text)
        .chars()
        .any(|c| c.is_ascii_digit())
}

pub fn prints_no_year(date_text: &str, date: Date) -> bool {
    reads_years_later(date_text, date, 4) || reads_years_later(date_text, date, -4)
}

fn reads_years_later(date_text: &str, date: Date, years: i32) -> bool {
    date.replace_year(date.year() + years)
        .is_ok_and(|shifted| date_matches(date_text, shifted))
}

pub fn nearest_readings(
    date_text: &str,
    date: Date,
    (from, to): (Date, Date),
    last_day: Option<Date>,
) -> Vec<Date> {
    let short_year = date.year().rem_euclid(100) as u8;
    let mut readings: Vec<Date> = (from.year() - 1..=to.year() + 1)
        .flat_map(|year| {
            let year_and_day_exchanged =
                (year.rem_euclid(100) == i32::from(date.day())).then_some(short_year);
            [Some(date.day()), year_and_day_exchanged]
                .into_iter()
                .flatten()
                .filter_map(move |day| Date::from_calendar_date(year, date.month(), day).ok())
        })
        .chain([date])
        .filter(|reading| last_day.is_none_or(|last_day| *reading <= last_day))
        .filter(|reading| date_matches(date_text, *reading))
        .collect();
    let outside = |reading: &Date| (from - *reading).max(*reading - to).max(Duration::ZERO);
    let nearest = readings.iter().map(outside).min();
    readings.retain(|reading| Some(outside(reading)) == nearest);
    readings
}

pub fn prints_day_first(date_text: &str, date: Date) -> Option<bool> {
    let text = normalize_digits(date_text);
    let text = TIME_OF_DAY.replace_all(&text, " ");
    if has_month_name(&text) {
        return None;
    }
    let numbers: Vec<&str> = NUMBERS.find_iter(&text).map(|m| m.as_str()).collect();
    let is_year = |digits: &str| {
        digits.parse() == Ok(date.year().rem_euclid(10_i32.pow(digits.len() as u32)))
    };
    let (first, second) = match numbers.as_slice() {
        [compact]
            if matches!(compact.len(), 6 | 8)
                && is_year(&compact[4..])
                && !(compact.len() == 6 && is_year(&compact[..2])) =>
        {
            (&compact[..2], &compact[2..4])
        }
        [first, second, ..] => (*first, *second),
        _ => return None,
    };
    let reads = |number: &str, value: u8| number.len() <= 2 && number.parse() == Ok(value);
    let (day, month) = (date.day(), u8::from(date.month()));
    if day == month {
        None
    } else if reads(first, day) && reads(second, month) {
        Some(true)
    } else if reads(first, month) && reads(second, day) {
        Some(false)
    } else {
        None
    }
}

pub fn runs_one_way(dates: &[Date]) -> bool {
    dates.windows(2).all(|pair| pair[0] <= pair[1])
        || dates.windows(2).all(|pair| pair[0] >= pair[1])
}

pub fn day_month_swapped(date: Date) -> Option<Date> {
    let month = Month::try_from(date.day()).ok()?;
    Date::from_calendar_date(date.year(), month, u8::from(date.month())).ok()
}

#[cfg(test)]
mod tests {
    use time::macros::{date, time};

    use super::*;

    #[test]
    fn dates_match_in_any_common_notation() {
        let day = date!(2024 - 01 - 02);
        for text in [
            "2024-01-02",
            "02/01/2024",
            "01/02/2024",
            "02.01.24",
            "2 Jan 2024",
            "Jan 2, 2024",
            "2nd January 2024",
            "02-Jan-2024",
            "20240102",
            "2024-01-02 08:12:40",
            "2024-01-02T10:00:00Z",
            "02.01.2024 10.15",
            "2024 m. sausio 2 d.",
            "2. Januar 2024",
            "2 janv. 2024",
            "2 de enero de 2024",
            "2 stycznia 2024",
            "2 января 2024",
            "2024年1月2日",
            "٢٠٢٤-٠١-٠٢",
            "02/01",
            "2 sausio",
            "240102",
            "020124",
            "2024년 1월 2일",
            "1月2日",
            "令和6年1月2日",
            "R6.1.2",
            "民國113年1月2日",
            "2567-01-02",
        ] {
            assert!(date_matches(text, day), "{text} should read as {day}");
        }
        assert!(date_matches("H29.09.01", date!(2017 - 09 - 01)));
        assert!(date_matches(
            "平成30年（2018年）7月15日",
            date!(2018 - 07 - 15)
        ));
        assert!(date_matches("2 Sept 2024", date!(2024 - 09 - 02)));
        assert!(date_matches("2 Dez. 2024", date!(2024 - 12 - 02)));
    }

    #[test]
    fn dates_that_differ_do_not_match() {
        let day = date!(2024 - 01 - 02);
        for text in [
            "2024-01-03",
            "03/01/2024",
            "2 Feb 2024",
            "2 vasario 2024",
            "2. Februar 2024",
            "2 Jan 2023",
            "12 Jan 2024",
            "Payment",
            "",
        ] {
            assert!(
                !date_matches(text, day),
                "{text:?} should not read as {day}"
            );
        }
    }

    #[test]
    fn date_shapes_are_found_in_any_script_and_notation() {
        for text in [
            "2024-01-02 Coffee 3.50",
            "02.01.2024",
            "14 Dec 2023 Bistro",
            "Dec 14, 2023",
            "02-Jan-2024",
            "20240102 TRANSFER",
            "2024年1月2日",
            "平成30年（2018年）7月15日",
            "1月2日 コンビニ",
            "2021-03-01T00:00:00",
            "2/22  Store #1  $2.05",
            "2 sausio",
            "3 Sept",
        ] {
            assert!(contains_date_shape(text), "{text}");
        }
        for text in ["Page 3 of 34", "Card ****1234", "Mark 3.50", "Total 12.50"] {
            assert!(!contains_date_shape(text), "{text}");
        }
    }

    #[test]
    fn printed_seconds_are_part_of_the_time() {
        assert!(time_appears("2024-01-02 14:14:20", time!(14:14:20)));
        assert!(time_appears("14:14:20.512", time!(14:14:20)));
        assert!(!time_appears("2024-01-02 14:14:20", time!(14:14)));
        assert!(!time_appears("2024-01-02 14:14:20", time!(14:14:21)));
        assert!(!time_appears("2024-01-02 14:14", time!(14:14:20)));
    }

    #[test]
    fn a_twelve_hour_clock_is_read_with_its_marker() {
        for (text, time) in [
            ("01/15/2024 2:30 PM", time!(14:30)),
            ("2:30pm", time!(14:30)),
            ("2:30:05 p.m.", time!(14:30:05)),
            ("9:05 AM", time!(09:05)),
            ("12:10 AM", time!(00:10)),
            ("12:10 PM", time!(12:10)),
            ("14:30 Amsterdam", time!(14:30)),
        ] {
            assert!(time_appears(text, time), "{text} should read as {time}");
        }
        assert!(!time_appears("01/15/2024 2:30 PM", time!(02:30)));
        assert!(!time_appears("12:10 AM", time!(12:10)));
    }

    #[test]
    fn a_word_beside_a_numeric_date_does_not_change_it() {
        let day = date!(2024 - 01 - 02);
        for text in [
            "Sat 02/01/2024",
            "Thu 02.01.2024",
            "2024-01-02 14:30:05 EST",
            "Mars 02/01/2024",
            "March 01/02/2024",
            "Fee 2024-01-02",
            "Sun 02/01",
            "Oct 20240102",
        ] {
            assert!(date_matches(text, day), "{text} should read as {day}");
        }
        assert!(!date_matches("Sat 03/01/2024", day));
        assert!(!date_matches("2 Mar 2024", day));
        assert!(!date_matches("2 Mars 24", date!(2024 - 01 - 02)));
        assert!(!has_month_name("Sat 02/01/2024"));
        assert!(has_month_name("2 Jan 2024"));
    }

    #[test]
    fn a_spreadsheet_day_number_is_a_date() {
        assert!(date_matches("45293", date!(2024 - 01 - 02)));
        assert!(date_matches("45293.75", date!(2024 - 01 - 02)));
        assert!(!date_matches("45293", date!(2024 - 01 - 03)));
        assert!(!date_matches("4529", date!(2024 - 01 - 02)));
    }

    #[test]
    fn the_printed_part_of_the_year_is_recognised() {
        let day = date!(2024 - 01 - 02);
        for (text, full, none) in [
            ("02/01/2024", true, false),
            ("2 Jan 2024", true, false),
            ("20240102", true, false),
            ("令和6年1月2日", true, false),
            ("45293", true, false),
            ("02/01/24", false, false),
            ("240102", false, false),
            ("02/01", false, true),
            ("2 Jan", false, true),
            ("Tue 02/01", false, true),
        ] {
            assert_eq!(prints_full_year(text, day), full, "{text}");
            assert_eq!(prints_no_year(text, day), none, "{text}");
        }
        assert!(prints_no_year("29 Feb", date!(2024 - 02 - 29)));
    }

    #[test]
    fn a_date_short_of_its_year_reads_nearest_the_documents_span() {
        let span = (date!(2023 - 12 - 01), date!(2024 - 01 - 31));
        assert_eq!(
            nearest_readings("28/12", date!(2019 - 12 - 28), span, None),
            vec![date!(2023 - 12 - 28)]
        );
        assert_eq!(
            nearest_readings("02/01", date!(2023 - 01 - 02), span, None),
            vec![date!(2024 - 01 - 02)]
        );
        assert_eq!(
            nearest_readings("28 Nov", date!(2024 - 11 - 28), span, None),
            vec![date!(2023 - 11 - 28)]
        );
        assert_eq!(
            nearest_readings("23/12/24", date!(2024 - 12 - 23), span, None),
            vec![date!(2023 - 12 - 24)]
        );
        assert_eq!(
            nearest_readings("23/12/24", date!(1923 - 12 - 24), span, None),
            vec![date!(2023 - 12 - 24)]
        );
        assert_eq!(
            nearest_readings("02/01/2019", date!(2019 - 01 - 02), span, None),
            vec![date!(2019 - 01 - 02)]
        );

        let year_end = date!(2024 - 12 - 31);
        assert_eq!(
            nearest_readings("02/01", date!(2024 - 01 - 02), (year_end, year_end), None),
            vec![date!(2025 - 01 - 02)]
        );
        assert_eq!(
            nearest_readings(
                "02/01",
                date!(2025 - 01 - 02),
                (year_end, year_end),
                Some(year_end)
            ),
            vec![date!(2024 - 01 - 02)]
        );
    }

    #[test]
    fn the_printed_day_month_order_is_recognised() {
        for (text, date, day_first) in [
            ("25/06/2024", date!(2024 - 06 - 25), Some(true)),
            ("06/25/2024", date!(2024 - 06 - 25), Some(false)),
            ("03/04/2024", date!(2024 - 04 - 03), Some(true)),
            ("03/04/2024", date!(2024 - 03 - 04), Some(false)),
            ("03/04", date!(2024 - 03 - 04), Some(false)),
            ("03042024", date!(2024 - 04 - 03), Some(true)),
            ("030424", date!(2024 - 04 - 03), Some(true)),
            ("240124", date!(2024 - 01 - 24), None),
            ("2024-04-03", date!(2024 - 04 - 03), None),
            ("20240403", date!(2024 - 04 - 03), None),
            ("24/04/03", date!(2024 - 04 - 03), None),
            ("3 Apr 2024", date!(2024 - 04 - 03), None),
            ("04/04/2024", date!(2024 - 04 - 04), None),
        ] {
            assert_eq!(prints_day_first(text, date), day_first, "{text} as {date}");
        }
    }

    #[test]
    fn a_timestamp_reads_as_the_date_and_time_it_prints() {
        for (text, day, time) in [
            (
                "2026-03-04T14:05:09Z",
                date!(2026 - 03 - 04),
                time!(14:05:09),
            ),
            (
                "2026-10-31T20:56:15+00:00",
                date!(2026 - 10 - 31),
                time!(20:56:15),
            ),
            (
                "2026-03-04T23:30:00-05:00",
                date!(2026 - 03 - 04),
                time!(23:30:00),
            ),
            (
                "2026-03-04T14:05:09.250+0100",
                date!(2026 - 03 - 04),
                time!(14:05:09),
            ),
            ("2026-03-04T14:05", date!(2026 - 03 - 04), time!(14:05)),
        ] {
            assert!(date_matches(text, day), "{text} should read as {day}");
            assert!(time_appears(text, time), "{text} should read as {time}");
        }
        assert!(!date_matches(
            "2026-03-04T23:30:00-05:00",
            date!(2026 - 03 - 05)
        ));
        assert!(!time_appears("2026-03-04T23:30:00-05:00", time!(04:30:00)));
        assert!(!date_matches("2026-03-04T14:05:09Z", date!(2014 - 05 - 09)));
    }

    #[test]
    fn a_date_packed_together_with_its_time_is_read() {
        for (text, day, time) in [
            (
                "20261031235959[0:GMT]",
                date!(2026 - 10 - 31),
                time!(23:59:59),
            ),
            (
                "20260304120000.000[-5:EST]",
                date!(2026 - 03 - 04),
                time!(12:00:00),
            ),
            ("20260304120000", date!(2026 - 03 - 04), time!(12:00:00)),
            ("20260304T140509Z", date!(2026 - 03 - 04), time!(14:05:09)),
            ("202603041405", date!(2026 - 03 - 04), time!(14:05)),
        ] {
            assert!(date_matches(text, day), "{text} should read as {day}");
            assert!(prints_full_year(text, day), "{text}");
            assert!(time_appears(text, time), "{text} should read as {time}");
            assert!(contains_date_shape(&format!("<DTPOSTED>{text}")), "{text}");
        }
        assert!(!date_matches(
            "20261031235959[0:GMT]",
            date!(2026 - 10 - 30)
        ));
        assert!(!time_appears("20261031235959[0:GMT]", time!(23:59)));
        assert!(!date_matches("20261331235959", date!(2026 - 01 - 31)));
        assert!(!contains_date_shape("Ref 20269931120000"));
    }

    #[test]
    fn digits_that_cannot_be_a_date_are_not_a_date_shape() {
        assert!(!contains_date_shape("Sort code 04-00-75"));
        assert!(contains_date_shape("Sort code 04-00-75 on 31.12.2024"));
        assert!(contains_date_shape("12/31/2024"));
        assert!(!contains_date_shape("2024-13-45"));
    }

    #[test]
    fn a_range_of_dates_reads_at_either_end() {
        for (text, from, to) in [
            (
                "April 1-30, 2026",
                date!(2026 - 04 - 01),
                date!(2026 - 04 - 30),
            ),
            (
                "1-30 April 2026",
                date!(2026 - 04 - 01),
                date!(2026 - 04 - 30),
            ),
            (
                "April 1 - May 2, 2026",
                date!(2026 - 04 - 01),
                date!(2026 - 05 - 02),
            ),
            (
                "01.04.-30.04.2026",
                date!(2026 - 04 - 01),
                date!(2026 - 04 - 30),
            ),
            (
                "13/12 - 12/01/2027",
                date!(2027 - 12 - 13),
                date!(2027 - 01 - 12),
            ),
        ] {
            for day in [from, to] {
                assert!(date_matches(text, day), "{text} should read as {day}");
                assert!(prints_full_year(text, day), "{text}");
            }
        }
        assert!(!date_matches("April 1-30, 2026", date!(2026 - 04 - 15)));
        assert!(!date_matches("April 1-30, 2026", date!(2025 - 04 - 30)));
        assert!(!date_matches("01.04.-30.04.2026", date!(2026 - 04 - 04)));
    }
}
