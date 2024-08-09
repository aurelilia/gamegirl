use std::marker::ConstParamTy;

use num_derive::FromPrimitive;

#[derive(FromPrimitive, ConstParamTy, PartialEq, Eq)]
pub enum ArmAluOp {
    And = 0,
    Eor,
    Sub,
    Rsb,
    Add,
    Adc,
    Sbc,
    Rsc,
    Tst,
    Teq,
    Cmp,
    Cmn,
    Orr,
    Mov,
    Bic,
    Mvn,
}

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum ArmMulOp {
    Mul,
    Mla,
    Umaal,
    Umull,
    Umlal,
    Smull,
    Smlal,
    SmlaXy,
    SmlawY,
    SmulwY,
    SmlalXy,
    SmulXy,
}

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum ArmQclzOp {
    Clz,
    Qadd,
    Qsub,
    QdAdd,
    QdSub,
}

#[derive(ConstParamTy, PartialEq, Eq)]
pub enum ArmLdrStrOp {
    Ldr,
    LdrH,
    LdrB,
    LdrSh,
    LdrSb,
    LdrD,
    Str,
    StrH,
    StrB,
    StrD,
}
