use itertools::izip;
use liquid_rust_core::ir::BasicBlock;
use rustc_span::Span;

use crate::{
    global_env::{GlobalEnv, Variance},
    pure_ctxt::PureCtxt,
    ty::{BaseTy, BinOp, Constr, ExprKind, Pred, Ty, TyKind, Var},
    type_env::TypeEnv,
};

pub struct ConstraintGen<'a, 'tcx> {
    genv: &'a GlobalEnv<'tcx>,
    pcx: PureCtxt<'a>,
    tag: Tag,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub enum Tag {
    Call(Span),
    Assign(Span),
    Ret,
    Div(Span),
    Rem(Span),
    Goto(Option<Span>, BasicBlock),
}

impl<'a, 'tcx> ConstraintGen<'a, 'tcx> {
    pub fn new(genv: &'a GlobalEnv<'tcx>, pcx: PureCtxt<'a>, tag: Tag) -> Self {
        ConstraintGen { genv, pcx, tag }
    }

    fn breadcrumb(&mut self) -> ConstraintGen<'_, 'tcx> {
        ConstraintGen { pcx: self.pcx.breadcrumb(), ..*self }
    }

    pub fn check_constr(&mut self, env: &TypeEnv, constr: &Constr) {
        match constr {
            Constr::Type(loc, ty) => {
                let actual_ty = env.lookup_loc(*loc);
                self.subtyping(actual_ty, ty.clone());
            }
            Constr::Pred(e) => {
                self.check_pred(e.clone());
            }
        }
    }

    pub fn check_pred(&mut self, pred: impl Into<Pred>) {
        self.pcx.push_head(pred, self.tag);
    }

    pub fn subtyping(&mut self, ty1: Ty, ty2: Ty) {
        let mut ck = self.breadcrumb();
        match (ty1.kind(), ty2.kind()) {
            (TyKind::Refine(bty1, e1), TyKind::Refine(bty2, e2)) if e1 == e2 => {
                ck.bty_subtyping(bty1, bty2);
                return;
            }
            (TyKind::Exists(bty1, p1), TyKind::Exists(bty2, p2)) if p1 == p2 => {
                ck.bty_subtyping(bty1, bty2);
                return;
            }
            (TyKind::Exists(bty, p), _) => {
                let fresh = ck
                    .pcx
                    .push_binding(ck.genv.sort(bty), |fresh| p.subst_bound_vars(Var::Free(fresh)));
                let ty1 = TyKind::Refine(bty.clone(), Var::Free(fresh).into()).intern();
                ck.subtyping(ty1, ty2);
                return;
            }
            _ => {}
        }

        match (ty1.kind(), ty2.kind()) {
            (TyKind::Refine(bty1, e1), TyKind::Refine(bty2, e2)) => {
                ck.bty_subtyping(bty1, bty2);
                ck.check_pred(ExprKind::BinaryOp(BinOp::Eq, e1.clone(), e2.clone()).intern());
            }
            (TyKind::Refine(bty1, e), TyKind::Exists(bty2, p)) => {
                ck.bty_subtyping(bty1, bty2);
                ck.check_pred(p.subst_bound_vars(e.clone()));
            }
            (TyKind::StrgRef(loc1), TyKind::StrgRef(loc2)) => {
                assert_eq!(loc1, loc2);
            }
            (TyKind::WeakRef(ty1), TyKind::WeakRef(ty2)) => {
                ck.subtyping(ty1.clone(), ty2.clone());
                ck.subtyping(ty2.clone(), ty1.clone());
            }
            (TyKind::ShrRef(ty1), TyKind::ShrRef(ty2)) => {
                ck.subtyping(ty1.clone(), ty2.clone());
            }
            (_, TyKind::Uninit) => {
                // FIXME: we should rethink in which situation this is sound.
            }
            (TyKind::Param(param1), TyKind::Param(param2)) => {
                debug_assert_eq!(param1, param2);
            }
            (TyKind::Exists(..), _) => unreachable!("subtyping with unpacked existential"),
            _ => todo!("`{ty1:?}` `{ty2:?}`"),
        }
    }

    fn bty_subtyping(&mut self, bty1: &BaseTy, bty2: &BaseTy) {
        match (bty1, bty2) {
            (BaseTy::Int(int_ty1), BaseTy::Int(int_ty2)) => {
                debug_assert_eq!(int_ty1, int_ty2);
            }
            (BaseTy::Uint(uint_ty1), BaseTy::Uint(uint_ty2)) => {
                debug_assert_eq!(uint_ty1, uint_ty2);
            }
            (BaseTy::Bool, BaseTy::Bool) => {}
            (BaseTy::Adt(did1, substs1), BaseTy::Adt(did2, substs2)) => {
                debug_assert_eq!(did1, did2);
                debug_assert_eq!(substs1.len(), substs2.len());
                let variances = self.genv.variances_of(*did1);
                for (variance, ty1, ty2) in izip!(variances, substs1.iter(), substs2.iter()) {
                    self.polymorphic_subtyping(*variance, ty1.clone(), ty2.clone());
                }
            }
            _ => unreachable!("unexpected base types: `{:?}` `{:?}`", bty1, bty2),
        }
    }

    fn polymorphic_subtyping(&mut self, variance: Variance, ty1: Ty, ty2: Ty) {
        match variance {
            rustc_middle::ty::Variance::Covariant => {
                self.subtyping(ty1, ty2);
            }
            rustc_middle::ty::Variance::Invariant => {
                self.subtyping(ty1.clone(), ty2.clone());
                self.subtyping(ty2, ty1);
            }
            rustc_middle::ty::Variance::Contravariant => {
                self.subtyping(ty2, ty1);
            }
            rustc_middle::ty::Variance::Bivariant => {}
        }
    }
}

mod pretty {
    use std::fmt;

    use super::*;
    use crate::pretty::*;

    impl Pretty for Tag {
        fn fmt(&self, _cx: &PPrintCx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            define_scoped!(_cx, f);
            match self {
                Tag::Call(span) => w!("Call({:?})", span),
                Tag::Assign(span) => w!("Assign({:?})", span),
                Tag::Ret => w!("Ret"),
                Tag::Div(span) => w!("Div({:?})", span),
                Tag::Rem(span) => w!("Rem({:?})", span),
                Tag::Goto(span, bb) => {
                    if let Some(span) = span {
                        w!("Goto({:?}, {:?})", span, ^bb)
                    } else {
                        w!("Goto({:?})", ^bb)
                    }
                }
            }
        }
    }
}