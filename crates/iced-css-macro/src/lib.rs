use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{Expr, ExprLit, ItemFn, Lit, LitStr, Token};

use iced_css_core::Stylesheet;
use iced_css_core::resolve::{Length, MarginValue, Resolved};

enum Source {
    Path(LitStr),
    Inline(LitStr),
}

struct Args {
    source: Source,
    /// The last segment of the policy path: `Compile`, `Policy::Compile` and
    /// `iced_css::Policy::Compile` all name the same policy.
    policy: syn::Ident,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let source = if input.peek(LitStr) {
            Source::Path(input.parse()?)
        } else {
            let key: syn::Ident = input.parse()?;
            if key != "inline" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected path or `inline = \"...\"`",
                ));
            }
            let _: Token![=] = input.parse()?;
            Source::Inline(input.parse()?)
        };

        let _: Token![,] = input.parse()?;
        let key: syn::Ident = input.parse()?;
        if key != "policy" {
            return Err(syn::Error::new(key.span(), "expected `policy = ...`"));
        }
        let _: Token![=] = input.parse()?;
        let path: syn::Path = input.parse()?;
        let policy = match path.segments.last() {
            Some(segment) if segment.arguments.is_none() => segment.ident.clone(),
            _ => return Err(syn::Error::new(path.span(), "expected a policy name")),
        };
        if input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
        }

        if !input.is_empty() {
            return Err(input.error("unexpected extra arguments"));
        }

        Ok(Args { source, policy })
    }
}

#[proc_macro_attribute]
pub fn style(args: TokenStream, item: TokenStream) -> TokenStream {
    // On a parse error, emit the item too, so its call sites do not also fail to resolve.
    let original = proc_macro2::TokenStream::from(item.clone());
    let args = match syn::parse::<Args>(args) {
        Ok(args) => args,
        Err(error) => {
            let error = error.into_compile_error();
            return quote!(#error #original).into();
        }
    };
    let mut function = match syn::parse::<ItemFn>(item) {
        Ok(function) => function,
        Err(error) => {
            let error = error.into_compile_error();
            return quote!(#error #original).into();
        }
    };

    match expand(&args, &mut function) {
        Ok(()) => quote!(#function).into(),
        Err(error) => {
            // Emit the item too, so unrelated diagnostics stay sane.
            let error = error.into_compile_error();
            quote!(#error #function).into()
        }
    }
}

fn expand(args: &Args, function: &mut ItemFn) -> syn::Result<()> {
    match args.policy.to_string().as_str() {
        "Compile" => {}
        "OnDemand" | "Auto" => {
            return Err(syn::Error::new(args.policy.span(), "not implemented yet"));
        }
        other => {
            return Err(syn::Error::new(
                args.policy.span(),
                format!("unknown policy `{other}`; expected Compile, OnDemand, or Auto"),
            ));
        }
    }

    let mut tracked = None;
    let mut sheet_path = None;
    let (css, css_span) = match &args.source {
        Source::Inline(lit) => (lit.value(), lit.span()),
        Source::Path(lit) => {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let path = std::path::Path::new(&manifest_dir).join(lit.value());
            match std::fs::read_to_string(&path) {
                Ok(css) => {
                    // `include_bytes!` resolves a relative path against the source file.
                    let absolute = std::path::absolute(&path).ok();
                    let Some(absolute) = absolute.as_deref().and_then(std::path::Path::to_str)
                    else {
                        return Err(syn::Error::new(
                            lit.span(),
                            format!("stylesheet path `{}` is not valid UTF-8", path.display()),
                        ));
                    };
                    tracked = Some(LitStr::new(absolute, lit.span()));
                    sheet_path = Some(path);
                    (css, lit.span())
                }
                Err(error) => {
                    return Err(syn::Error::new(
                        lit.span(),
                        format!("cannot read stylesheet `{}`: {error}", path.display()),
                    ));
                }
            }
        }
    };

    let stylesheet = iced_css_core::parse(&css).map_err(|error| {
        // The span is the path literal; name the file the line and column refer to.
        let message = match &sheet_path {
            Some(path) => format!("in stylesheet `{}`: {error}", path.display()),
            None => error.to_string(),
        };
        syn::Error::new(css_span, message)
    })?;

    let mut rewriter = Rewriter {
        stylesheet,
        errors: Vec::new(),
        rewrote: false,
    };
    // The annotation owns this body, not public signature expressions.
    rewriter.visit_block_mut(&mut function.block);

    if rewriter.rewrote {
        // Imported once for the whole body, so a rewritten call needs no block of its own:
        // a block's tail would drop the receiver's temporaries at the end of the block.
        function.block.stmts.insert(
            0,
            syn::parse_quote!(
                #[allow(unused_imports)]
                use ::iced_css::dispatch::{TagButton as _, TagContainer as _, TagWrapped as _};
            ),
        );
    }

    if let Some(path) = tracked {
        // Reading the file here tells the compiler nothing. An unused `include_bytes!` makes
        // the stylesheet an input of the build, so editing it alone rebuilds the function.
        function.block.stmts.insert(
            0,
            syn::parse_quote!(const _: &[u8] = ::core::include_bytes!(#path);),
        );
    }

    match rewriter.errors.into_iter().reduce(|mut all, error| {
        all.combine(error);
        all
    }) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct Rewriter {
    stylesheet: Stylesheet,
    errors: Vec<syn::Error>,
    rewrote: bool,
}

/// The first `style_classes` identifier in a macro body, however deeply it is grouped.
fn style_classes_span(tokens: proc_macro2::TokenStream) -> Option<Span> {
    tokens.into_iter().find_map(|token| match token {
        proc_macro2::TokenTree::Ident(ident) if ident == "style_classes" => Some(ident.span()),
        proc_macro2::TokenTree::Group(group) => style_classes_span(group.stream()),
        _ => None,
    })
}

impl VisitMut for Rewriter {
    // Nested functions, impls, constants and other items have independent scope.
    // Rust expands any annotations on those items separately.
    fn visit_item_mut(&mut self, _item: &mut syn::Item) {}

    // Reached for a macro in expression position and in statement position alike: a
    // trailing `;` or a brace delimiter makes the call a statement, not an expression.
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        use syn::punctuated::Punctuated;
        match mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
            Ok(mut elements) => {
                for element in elements.iter_mut() {
                    self.visit_expr_mut(element);
                }
                mac.tokens = quote!(#elements);
            }
            Err(_) => {
                // Left alone, the call would fail on the trait's `Rewritten` bound, less clearly.
                if let Some(span) = style_classes_span(mac.tokens.clone()) {
                    self.errors.push(syn::Error::new(
                        span,
                        "cannot rewrite `style_classes` inside this macro call: its body is \
                         not a comma-separated list of expressions; move the call out of the macro",
                    ));
                }
            }
        }
    }

    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        syn::visit_mut::visit_expr_mut(self, expr);

        let Expr::MethodCall(call) = expr else {
            return;
        };
        if call.method != "style_classes" || call.args.len() != 1 {
            return;
        }

        // Keep diagnostics inside the expression's existing compiler guard ancestry.
        // Move attributes to its replacement once; do not evaluate cfg here.
        let attrs = std::mem::take(&mut call.attrs);
        let receiver = &call.receiver;
        let resolved = literal_classes(&call.args[0]).map(|classes| {
            let names: Vec<&str> = classes.iter().map(|(name, _)| name.as_str()).collect();
            resolved_tokens(&self.stylesheet.resolve(&names))
        });
        *expr = match resolved {
            Ok(resolved) => {
                self.rewrote = true;
                // `match` keeps the receiver's temporaries alive to the end of the enclosing
                // statement, as the original method call did; a `let` would drop them.
                let mut replacement: syn::ExprMatch = syn::parse_quote! {
                    match #receiver {
                        __iced_css_widget => (&__iced_css_widget)
                            .__iced_css_tag()
                            .apply(__iced_css_widget, #resolved),
                    }
                };
                replacement.attrs = attrs;
                Expr::Match(replacement)
            }
            Err(error) => {
                let diagnostic = error.into_compile_error();
                let mut replacement: syn::ExprBlock = syn::parse_quote!({ #diagnostic #receiver });
                replacement.attrs = attrs;
                Expr::Block(replacement)
            }
        };
    }
}

fn literal_classes(arg: &Expr) -> syn::Result<Vec<(String, Span)>> {
    let Expr::Array(array) = arg else {
        return Err(syn::Error::new(
            arg.span(),
            "under the Compile policy, style_classes takes an array of string literals",
        ));
    };

    array
        .elems
        .iter()
        .map(|element| match element {
            Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) => Ok((lit.value(), lit.span())),
            other => Err(syn::Error::new(
                other.span(),
                "under the Compile policy, class names must be string literals",
            )),
        })
        .collect()
}

fn resolved_tokens(resolved: &Resolved) -> proc_macro2::TokenStream {
    let length = |value: &Option<Length>| match value {
        None => quote!(::core::option::Option::None),
        Some(Length::Px(v)) => quote!(::core::option::Option::Some(::iced_css::Length::Px(#v))),
        Some(Length::Percent(v)) => {
            quote!(::core::option::Option::Some(::iced_css::Length::Percent(#v)))
        }
        Some(Length::Auto) => quote!(::core::option::Option::Some(::iced_css::Length::Auto)),
    };
    let margin_value = |value: &MarginValue| match value {
        MarginValue::Px(v) => quote!(::iced_css::MarginValue::Px(#v)),
        MarginValue::Auto => quote!(::iced_css::MarginValue::Auto),
    };

    let width = length(&resolved.width);
    let height = length(&resolved.height);
    let min_width = length(&resolved.min_width);
    let max_width = length(&resolved.max_width);
    let min_height = length(&resolved.min_height);
    let max_height = length(&resolved.max_height);
    let padding = match &resolved.padding {
        None => quote!(::core::option::Option::None),
        Some(p) => {
            let (top, right, bottom, left) = (p.top, p.right, p.bottom, p.left);
            quote!(::core::option::Option::Some(::iced_css::Paddings {
                top: #top,
                right: #right,
                bottom: #bottom,
                left: #left,
            }))
        }
    };
    let margin = match &resolved.margin {
        None => quote!(::core::option::Option::None),
        Some(margins) => {
            let top = margin_value(&margins.top);
            let right = margin_value(&margins.right);
            let bottom = margin_value(&margins.bottom);
            let left = margin_value(&margins.left);
            quote!(::core::option::Option::Some(::iced_css::Margins {
                top: #top,
                right: #right,
                bottom: #bottom,
                left: #left,
            }))
        }
    };

    quote!(::iced_css::Resolved {
        width: #width,
        height: #height,
        min_width: #min_width,
        max_width: #max_width,
        min_height: #min_height,
        max_height: #max_height,
        margin: #margin,
        padding: #padding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn expand_str(
        args: proc_macro2::TokenStream,
        function: proc_macro2::TokenStream,
    ) -> Result<String, String> {
        let args: Args = syn::parse2(args).map_err(|e| e.to_string())?;
        let mut function: ItemFn = syn::parse2(function).expect("test fn must parse");
        expand(&args, &mut function)
            .map(|()| function.to_token_stream().to_string())
            .map_err(|e| e.to_string())
    }

    fn view() -> proc_macro2::TokenStream {
        quote! {
            fn view() -> Element {
                container(text("t")).style_classes(["btn"])
            }
        }
    }

    #[test]
    fn valid_usage_rewrites_to_apply() {
        let out = expand_str(
            quote!(
                inline = ".btn { width: 50px; height: 50px; }",
                policy = Compile
            ),
            view(),
        )
        .unwrap();
        assert!(out.contains("__iced_css_tag"), "{out}");
        assert!(out.contains("Px (50f32)"), "{out}");
        assert!(!out.contains("style_classes"), "{out}");
    }

    #[test]
    fn rewrites_inside_column_macro() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn view() -> Element {
                    iced::widget::column![
                        container(text("a")).style_classes(["btn"]),
                        container(text("b")),
                    ].into()
                }
            },
        )
        .unwrap();
        assert!(out.contains("__iced_css_tag"), "{out}");
        assert!(!out.contains("style_classes"), "{out}");
    }

    #[test]
    fn rewrites_inside_statement_macros() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn check() {
                    assert_eq!(width(container(text("a")).style_classes(["btn"])), 50.);
                    block! { container(text("b")).style_classes(["btn"]) }
                }
            },
        )
        .unwrap();
        assert_eq!(out.matches("__iced_css_tag").count(), 2, "{out}");
        assert!(!out.contains("style_classes"), "{out}");
    }

    #[test]
    fn unparseable_macro_body_with_style_classes_is_a_build_error() {
        for body in [
            quote!(vec![container(text("a")).style_classes(["btn"]); 1]),
            quote!(vec![container(text("a")).style_classes(["btn"]); 1];),
            quote!(map! { "a" => { container(text("a")).style_classes(["btn"]) } }),
        ] {
            let err = expand_str(
                quote!(inline = ".btn { width: 50px; }", policy = Compile),
                quote! {
                    fn check() {
                        #body
                    }
                },
            )
            .unwrap_err();
            assert!(err.contains("cannot rewrite `style_classes`"), "{err}");
        }
    }

    #[test]
    fn unparseable_macro_body_without_style_classes_is_left_alone() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn check() {
                    let sizes = vec![0u8; 4];
                    container(text("a")).style_classes(["btn"])
                }
            },
        )
        .unwrap();
        assert!(out.contains("vec ! [0u8 ; 4]"), "{out}");
        assert!(out.contains("__iced_css_tag"), "{out}");
    }

    #[test]
    fn invalid_css_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; height:", policy = Compile),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("CSS parse error"), "{err}");
    }

    #[test]
    fn invalid_css_in_a_file_names_the_file() {
        let path =
            std::env::temp_dir().join(format!("iced-css-macro-invalid-{}.css", std::process::id()));
        std::fs::write(&path, ".btn { width: 50px; height:").unwrap();
        let literal = path.to_str().unwrap();
        let err = expand_str(quote!(#literal, policy = Compile), view());
        std::fs::remove_file(&path).unwrap();
        let err = err.unwrap_err();
        assert!(err.contains("CSS parse error"), "{err}");
        assert!(err.contains(&format!("`{}`", path.display())), "{err}");
    }

    #[test]
    fn rewrite_keeps_receiver_temporaries_in_one_match() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            view(),
        )
        .unwrap();
        let function: ItemFn = syn::parse_str(&out).unwrap();
        let first = function.block.stmts[0].to_token_stream().to_string();
        assert!(first.contains("TagWrapped"), "{first}");
        assert!(out.contains("match container"), "{out}");
        assert!(!out.contains("let __iced_css_widget"), "{out}");
    }

    #[test]
    fn body_without_style_classes_gets_no_import() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote!(
                fn view() -> u8 {
                    1
                }
            ),
        )
        .unwrap();
        assert!(!out.contains("dispatch"), "{out}");
    }

    #[test]
    fn missing_file_is_a_build_error() {
        let err = expand_str(quote!("does/not/exist.css", policy = Compile), view()).unwrap_err();
        assert!(err.contains("cannot read stylesheet"), "{err}");
    }

    #[test]
    fn path_stylesheet_is_a_tracked_build_input() {
        let path = std::env::temp_dir().join(format!(
            "iced-css-macro-tracked-{}.css",
            std::process::id()
        ));
        std::fs::write(&path, ".btn { width: 50px; }").unwrap();
        let literal = path.to_str().unwrap();
        let out = expand_str(quote!(#literal, policy = Compile), view());
        std::fs::remove_file(&path).unwrap();
        let out = out.unwrap();

        let function: ItemFn = syn::parse_str(&out).unwrap();
        let first = function.block.stmts[0].to_token_stream().to_string();
        assert!(first.contains("include_bytes"), "{first}");
        let tracked: LitStr = syn::parse_quote!(#literal);
        assert!(
            first.contains(&tracked.to_token_stream().to_string()),
            "{first}"
        );
        assert!(out.contains("50f32"), "{out}");
    }

    #[test]
    fn inline_stylesheet_tracks_no_file() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            view(),
        )
        .unwrap();
        assert!(!out.contains("include_bytes"), "{out}");
    }

    #[test]
    fn runtime_policies_are_not_implemented_yet() {
        for policy in [quote!(OnDemand), quote!(Auto)] {
            let err = expand_str(
                quote!(inline = ".btn { width: 50px; }", policy = #policy),
                view(),
            )
            .unwrap_err();
            assert!(err.contains("not implemented yet"), "{err}");
        }
    }

    #[test]
    fn unknown_policy_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Elsex),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("unknown policy `Elsex`"), "{err}");
    }

    #[test]
    fn policy_may_be_a_path_with_a_trailing_comma() {
        for args in [
            quote!(inline = ".btn { width: 50px; }", policy = Policy::Compile),
            quote!(
                inline = ".btn { width: 50px; }",
                policy = ::iced_css::Policy::Compile,
            ),
            quote!(inline = ".btn { width: 50px; }", policy = Compile,),
        ] {
            let out = expand_str(args, view()).unwrap();
            assert!(out.contains("__iced_css_tag"), "{out}");
        }
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Policy::Elsex),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("unknown policy `Elsex`"), "{err}");
    }

    #[test]
    fn malformed_arguments_are_rejected() {
        assert!(expand_str(quote!(inline = ".btn {}"), view()).is_err());
        assert!(expand_str(quote!(inlined = ".btn {}", policy = Compile), view()).is_err());
        assert!(
            expand_str(
                quote!(inline = ".btn {}", policy = Compile, extra = 1),
                view()
            )
            .is_err()
        );
        assert!(expand_str(quote!(inline = ".btn {}", policy = Compile,,), view()).is_err());
        assert!(expand_str(quote!(inline = ".btn {}", policy = Vec<u8>), view()).is_err());
    }
}
