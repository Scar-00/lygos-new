use lygos_llvm::{types::TypeRef, value::CastOps};
use lygos_logging::{ErrorLabel, Logger};
use lygos_macros::Visitable;
use lygos_token::{Location, Tagged};

use crate::{types::Type, AST};

#[derive(Debug, Visitable)]
pub struct AssignmentExpr {
    pub lhs: Box<AST>,
    pub rhs: Box<AST>,
}

impl AssignmentExpr {
    pub fn new(lhs: Box<AST>, rhs: Box<AST>) -> Self {
        Self{ lhs, rhs }
    }
}

#[derive(Debug, Visitable)]
pub struct UnaryExpr {
    pub op: Tagged<String>,
    pub obj: Box<AST>,
}

impl UnaryExpr {
    pub fn new(op: Tagged<String>, obj: Box<AST>) -> Self {
        Self{ op, obj }
    }
}

#[derive(Debug, Visitable)]
pub struct CastExpr {
    pub obj: Box<AST>,
    pub target_type: Type,
}

impl CastExpr {
    pub fn new(obj: Box<AST>, target_type: Type) -> Self {
        Self{ obj, target_type }
    }

    pub fn get_cast_ops(src_loc: &Location, src: &TypeRef, dest_loc: &Location, dest: &TypeRef) -> CastOps {
        if src.is_int_ty() && dest.is_int_ty() {
            if src.get_int_bit_width() > dest.get_int_bit_width() {
                return CastOps::Trunc;
            }
            return CastOps::SExt;
        }
        if src.is_pointer_ty() && dest.is_pointer_ty() {
            return CastOps::BitCast;
        }
        if src.is_pointer_ty() && dest.is_int_ty() {
            return CastOps::PtrToInt;
        }
        if src.is_float_ty() && dest.is_float_ty() && src.is_float_ty() && dest.is_double_ty() {
            return CastOps::FPExt;
        }
        if src.is_int_ty() {
            if dest.is_float_ty() {
                return CastOps::SIToFP;
            }
            if dest.is_pointer_ty() {
                return CastOps::IntToPtr;
            }
        }
        if src.is_struct_ty() && dest.is_struct_ty() && src.can_losslessly_bit_cast_to(*dest) {
            return CastOps::BitCast;
        }
        Logger::error_msg_labels("unable to convert types", &[
            ErrorLabel::from(src_loc, format!("`from {}`", src.print()).as_str()),
            ErrorLabel::from(dest_loc, format!("`to {}`", dest.print()).as_str()),
        ]);
    }
}
