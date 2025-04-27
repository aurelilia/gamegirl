use alloc::{boxed::Box, string::ToString};

use common::testing::{self, TestStatus};

use crate::GameGirlAdv;

#[test]
fn jsmolka_arm() {
    testing::run_test(include_bytes!("jsmolka/arm.gba"), inspect_jsmolka);
}

#[test]
fn jsmolka_bios() {
    testing::run_test(include_bytes!("jsmolka/bios.gba"), inspect_jsmolka);
}

#[test]
fn jsmolka_memory() {
    testing::run_test(include_bytes!("jsmolka/memory.gba"), inspect_jsmolka);
}

#[test]
fn jsmolka_nes() {
    testing::run_test(include_bytes!("jsmolka/nes.gba"), inspect_jsmolka);
}

#[test]
fn jsmolka_thumb() {
    testing::run_test(include_bytes!("jsmolka/thumb.gba"), inspect_jsmolka);
}

#[test]
fn jsmolka_unsafe() {
    testing::run_test(include_bytes!("jsmolka/unsafe.gba"), inspect_jsmolka);
}

#[test]
fn fuzzarm_arm_any() {
    testing::run_test(include_bytes!("fuzzarm/ARM_Any.gba"), inspect_fuzzarm);
}

#[test]
fn fuzzarm_arm_dataprocessing() {
    testing::run_test(
        include_bytes!("fuzzarm/ARM_DataProcessing.gba"),
        inspect_fuzzarm,
    );
}

#[test]
fn fuzzarm_all() {
    testing::run_test(include_bytes!("fuzzarm/FuzzARM.gba"), inspect_fuzzarm);
}

#[test]
fn fuzzarm_thumb_any() {
    testing::run_test(include_bytes!("fuzzarm/THUMB_Any.gba"), inspect_fuzzarm);
}

#[test]
fn fuzzarm_thumb_dataprocessing() {
    testing::run_test(
        include_bytes!("fuzzarm/THUMB_DataProcessing.gba"),
        inspect_fuzzarm,
    );
}

fn inspect_jsmolka(gg: &mut Box<GameGirlAdv>) -> TestStatus {
    let hash = testing::screen_hash(gg);
    let regs = &gg.cpu.state.registers;

    if regs[13] == 0x03008014 {
        let ones = regs[10];
        let tens = regs[9];
        let hundreds = regs[8];
        let test = ones + (tens * 10) + (hundreds * 100);
        TestStatus::FailedAt(test.to_string())
    } else if [
        0x20974E0091874964,
        0x94F4D344B975EB0C,
        0x1A8992654BCDC4D8,
        0x63E68B6E5115B556,
    ]
    .contains(&hash)
    {
        TestStatus::Success
    } else {
        TestStatus::Running
    }
}

pub fn inspect_fuzzarm(gg: &mut Box<GameGirlAdv>) -> TestStatus {
    if testing::screen_hash(gg) == 0xD5170621BA472629 {
        TestStatus::Success
    } else {
        TestStatus::Running
    }
}
