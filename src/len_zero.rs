extern crate rustc_typeck as typeck;

use std::rc::Rc;
use std::cell::RefCell;
use syntax::ptr::P;
use rustc::lint::{Context, LintPass, LintArray, Lint};
use rustc::util::nodemap::DefIdMap;
use rustc::middle::ty::{self, node_id_to_type, sty, ty_ptr, ty_rptr, expr_ty,
	mt, ty_to_def_id, impl_or_trait_item, MethodTraitItemId, ImplOrTraitItemId};
use rustc::middle::def::{DefTy, DefStruct, DefTrait};
use syntax::codemap::{Span, Spanned};
use syntax::ast::*;

declare_lint!(pub LEN_ZERO, Warn,
              "Warn on usage of double-mut refs, e.g. '&mut &mut ...'");

declare_lint!(pub LEN_WITHOUT_IS_EMPTY, Warn,
              "Warn on traits and impls that have .len() but not .is_empty()");

#[derive(Copy,Clone)]
pub struct LenZero;

impl LintPass for LenZero {
	fn get_lints(&self) -> LintArray {
        lint_array!(LEN_ZERO, LEN_WITHOUT_IS_EMPTY)
	}
	
	fn check_item(&mut self, cx: &Context, item: &Item) {
		match &item.node {
			&ItemTrait(_, _, _, ref trait_items) => 
				check_trait_items(cx, item, trait_items),
			&ItemImpl(_, _, _, None, _, ref impl_items) => // only non-trait
				check_impl_items(cx, item, impl_items),
			_ => ()
		}
	}
	
	fn check_expr(&mut self, cx: &Context, expr: &Expr) {
		if let &ExprBinary(Spanned{node: cmp, ..}, ref left, ref right) = 
				&expr.node {
			match cmp {
				BiEq => check_cmp(cx, expr.span, left, right, ""),
				BiGt | BiNe => check_cmp(cx, expr.span, left, right, "!"),
				_ => ()
			}
		}
	}
}

fn check_trait_items(cx: &Context, item: &Item, trait_items: &[P<TraitItem>]) {
	fn is_named_self(item: &TraitItem, name: &str) -> bool {
		item.ident.as_str() == name && item.attrs.len() == 0
	}

	if !trait_items.iter().any(|i| is_named_self(i, "is_empty")) {
		//cx.span_lint(LEN_WITHOUT_IS_EMPTY, item.span, &format!("trait {}", item.ident.as_str()));
		for i in trait_items {
			if is_named_self(i, "len") {
				cx.span_lint(LEN_WITHOUT_IS_EMPTY, i.span,
					&format!("Trait '{}' has a '.len()' method, but no \
						'.is_empty()' method. Consider adding one.", 
						item.ident.as_str()));
			}
		};
	}
}

fn check_impl_items(cx: &Context, item: &Item, impl_items: &[P<ImplItem>]) {
	fn is_named_self(item: &ImplItem, name: &str) -> bool {
		item.ident.as_str() == name && item.attrs.len() == 0
	}

	if !impl_items.iter().any(|i| is_named_self(i, "is_empty")) {
		for i in impl_items {
			if is_named_self(i, "len") {
				let s = i.span;
				cx.span_lint(LEN_WITHOUT_IS_EMPTY, 
					Span{ lo: s.lo, hi: s.lo, expn_id: s.expn_id },
					&format!("Item '{}' has a '.len()' method, but no \
						'.is_empty()' method. Consider adding one.", 
						item.ident.as_str()));
				return;
			}
		}
	}
}

fn check_cmp(cx: &Context, span: Span, left: &Expr, right: &Expr, empty: &str) {
	match (&left.node, &right.node) {
		(&ExprLit(ref lit), &ExprMethodCall(ref method, _, ref args)) => 
			check_len_zero(cx, span, method, args, lit, empty),
		(&ExprMethodCall(ref method, _, ref args), &ExprLit(ref lit)) => 
			check_len_zero(cx, span, method, args, lit, empty),
		_ => ()
	}
}

fn check_len_zero(cx: &Context, span: Span, method: &SpannedIdent, 
		args: &[P<Expr>], lit: &Lit, empty: &str) {
	if let &Spanned{node: LitInt(0, _), ..} = lit {
		if method.node.as_str() == "len" && args.len() == 1 &&
			has_is_empty(cx, &expr_ty(cx.tcx, &*args[0])) {
			cx.span_lint(LEN_ZERO, span, &format!(
				"Consider replacing the len comparison with \
				'{}_.is_empty()' if available",
					empty))
		}
	}
}

fn has_is_empty(cx: &Context, ty: &::rustc::middle::ty::Ty) -> bool {
	fn check_item(cx: &Context, id: &ImplOrTraitItemId) -> bool {
		if let &MethodTraitItemId(ref def_id) = id {
			if let ty::MethodTraitItem(ref method) = ty::impl_or_trait_item(
					cx.tcx, *def_id) {
				method.name.as_str() == "is_empty"
			} else { false }
		} else { false }
	}
	
	::rustc::middle::ty::ty_to_def_id(ty).map_or(true, |id| {
		cx.tcx.impl_items.borrow().get(&id).map_or(false, |item_ids| {
			item_ids.iter().any(|i| check_item(cx, i))
		}) || cx.tcx.trait_item_def_ids.borrow().get(&id).map_or(false,
			|item_ids| { item_ids.iter().any(|i| check_item(cx, i)) })
	})
}
