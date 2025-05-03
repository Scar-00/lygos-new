use proc_macro::{self, TokenStream};
use syn::{Data, Fields};
use quote::{quote, format_ident};

#[proc_macro_derive(Visitable)]
pub fn visitable_impl(items: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(items as syn::DeriveInput);
    let ident = input.ident;
    let fn_ident = format_ident!("visit_{}", ident.to_string().to_lowercase());
    quote! {
        impl #ident {
            pub fn accept<V: crate::ASTVisitor>(&mut self, visitor: &mut V) -> V::Output {
                visitor.#fn_ident(self)
            }
        }
    }.into()
}

#[proc_macro_derive(Visitor)]
pub fn visitor(items: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(items as syn::DeriveInput);
    let ident = input.ident;
    let mut funcs = Vec::new();
    let mut types = Vec::new();
    if let Data::Enum(e) = input.data {
        for member in e.variants {
            //assert!(member.fields.len() == 1);
            let fn_ident = format_ident!("visit_{}", member.ident.to_string().to_lowercase());
            if let Fields::Unnamed(field) = member.fields {
                if let Some(field) = field.unnamed.first() {
                    types.push(field.ty.clone());
                }
            }
            funcs.push(fn_ident);
            //let ty = member.fields.members().next().unwrap();
        }
    }

    //assert!(funcs.len() == types.len());

    let funcs = funcs.iter();
    let types = types.iter();
    let ident = format_ident!("{}Visitor", ident);
    quote! {
        pub trait #ident {
            type Output;
            #(fn #funcs(&mut self, node: &mut #types) -> Self::Output;)*
        }
    }.into()
}

#[proc_macro_derive(VisitorImpl, attributes(visitor))]
pub fn visitor_impl(items: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(items as syn::DeriveInput);
    let ident = input.ident;
    let fn_ident = format_ident!("visit_{}", ident.to_string().to_lowercase());
    //println!("{}", input.attrs[0].meta.require_list().unwrap().tokens);
    if let Data::Struct(_) = input.data {
        if let Ok(list) = input.attrs[0].meta.require_list() {
            if let Some(tok) = list.tokens.clone().into_iter().next() {
                let res = quote! {
                    impl #ident {
                        pub fn accept(&mut self, visitor: &mut dyn #tok) -> #tok::Output {
                            visitor.#fn_ident(self);
                        }
                    }
                }.into();
                //println!("{}", res);
                return res;
            }
        }
    }
    if let Data::Enum(e) = input.data {
        let mut arms = Vec::new();
        for member in e.variants {
            //assert!(member.fields.len() == 1);
            let fn_ident = format_ident!("visit_{}", member.ident.clone().to_string().to_lowercase());
            let member_ident = member.ident;
            arms.push(quote! {
                #ident::#member_ident(v) => visitor.#fn_ident(v),
            });
        }
        let arms = arms.iter();
        if let Ok(list) = input.attrs[0].meta.require_list() {
            if let Some(tok) = list.tokens.clone().into_iter().next() {
                let res = quote! {
                    impl #ident {
                        pub fn accept<V: #tok>(&mut self, visitor: &mut V) -> V::Output {
                            match self {
                                #(#arms)*
                            }
                        }
                    }
                }.into();
                //println!("{}", res);
                return res;
            }
        }
    }

    panic!();
}
