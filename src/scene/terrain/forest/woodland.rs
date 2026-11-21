//! Small pen-and-wash specimens. Coordinates are local to the root pivot.
use std::fmt::Write;

pub(super) fn specimen(variant: usize) -> String {
    if super::SCRUB.contains(&variant) {
        return super::specimens::scrub(variant)
            .replace("#96805a", "#757950")
            .replace("#c6a67c", "#96996b")
            .replace("#e2c799", "#c3be8c");
    }
    let fir = variant < 6 || super::FIR.contains(&variant);
    let height = if fir { 41 } else { 33 } + (variant % 4) * 2;
    let spread = if fir { 9 } else { 14 } + (variant % 3);
    let lean = variant as i32 % 5 - 2;
    let mut svg = String::from(
        r##"<path d="M-8 1Q-3-3 5-2 15-2 23 4 16 7 6 6L-3 4Z" fill="#897957" opacity=".15"/>
<path d="M-5 1Q0-1 6 1L9 3Q2 4-5 2Z" fill="#66563e" opacity=".24"/>
<path d="M-3 2Q-.9-2-1.7-13L2.8-15Q1.7-3 4 1L1 0Z" fill="#9f8866" stroke="#5d4b3b" stroke-width=".85" stroke-linejoin="round"/>
<path d="M6 3l3-1m5 3 3-1M-8 2l2-.7" fill="none" stroke="#786b4c" stroke-width=".68" opacity=".46" stroke-linecap="round"/>"##,
    );
    let spread = spread as i32;
    let height = height as i32;
    if fir {
        let tip = -height;
        let shoulder = -height * 2 / 3;
        let middle = -height / 2;
        let left = -spread;
        let right = spread + 1;
        write!(
            svg,
            r##"<path d="M{lean} {tip}Q1 {shoulder} {right} {middle}L5 -21Q8 -16 {right} -12L7 -13 10 -8Q4-6 0-9 -5-6 {left} -9L-5-15 {left} -14Q-6-19-4-23L-8-22Q-2 {shoulder} {lean} {tip}Z" fill="#81906a" stroke="#5d4b3b" stroke-width=".92" stroke-linejoin="round"/>
<path d="M{lean} {tip}Q2 {shoulder} {right} {middle}L5-21Q8-16 {right}-12L7-13 10-8Q4-6 0-9L2-18 0-16 1-25Z" fill="#5e6b4c"/>
<path d="M{lean} {tip}Q-2 {shoulder} -7 -23L-2-25Q-5-19-8-16L-3-18-5-13-1-15-2-21 0-28Z" fill="#b3b68b"/>
<path d="M-5-20q4 0 6-3m-8 9q4 1 8-2m1-9 4 4m-2 5 5 3M-6-9q5 1 8-2" fill="none" stroke="#5d4b3b" stroke-width=".64" opacity=".55" stroke-linecap="round"/>"##
        )
        .unwrap();
    } else {
        let top = -height;
        let right = spread + 3;
        let left = -spread;
        write!(
            svg,
            r##"<path d="M{left}-21Q{left}-27 -8-28 -6 {top} {lean} {top} 7 {top} 9-28 {right}-28 {right}-21 20-15 15-11 11-12 8-8 2-8-1-11-7-8-12-12-19-12 {left}-21Z" fill="#a5a67e" stroke="#5d4b3b" stroke-width=".95" stroke-linejoin="round"/>
<path d="M{right}-22Q19-15 15-11 11-12 8-8 2-8-1-11-7-8-12-12-19-12 {left}-21Q-11-24-8-20-7-14-3-15-1-21 5-20 9-17 12-21 15-24 {right}-22Z" fill="#777f5c"/>
<path d="M-12-23Q-10-28-5-28-3 {top} {lean} {top} 5 {top} 7-29 4-27 2-24-3-26-6-23-8-20-12-23Z" fill="#c3c29a"/>
<path d="M-7-24q2-3 6-2m2 7q3-4 7-3m-17 7q3 2 6 0m8 2q4-3 7-2m1-12q-3-1-6 1" fill="none" stroke="#5d4b3b" stroke-width=".66" opacity=".53" stroke-linecap="round"/>"##
        )
        .unwrap();
    }
    svg
}
