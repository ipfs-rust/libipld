use crate::ast::*;
use proc_macro2::TokenStream;
use quote::quote;

pub fn gen_encode(ast: &SchemaType) -> TokenStream {
    let (ident, body) = match ast {
        SchemaType::Struct(s) => (&s.name, gen_encode_struct(&s)),
        SchemaType::Union(u) => (&u.name, gen_encode_union(&u)),
    };

    quote! {
        impl libipld::codec::Encode<libipld::cbor::DagCborCodec> for #ident {
            fn encode<W: std::io::Write>(
                &self,
                c: libipld::cbor::DagCborCodec,
                w: &mut W,
            ) -> libipld::Result<()> {
                use libipld::codec::Encode;
                use libipld::cbor::encode::{write_null, write_u64};
                #body
            }
        }
    }
}

pub fn gen_decode(ast: &SchemaType) -> TokenStream {
    let ident = match ast {
        SchemaType::Struct(s) => &s.name,
        SchemaType::Union(u) => &u.name,
    };

    quote! {
        impl libipld::codec::Decode<libipld::cbor::DagCborCodec> for #ident {
            fn decode<R: std::io::Read>(
                c: libipld::cbor::DagCborCodec,
                r: &mut R,
            ) -> libipld::Result<Self> {
                libipld::cbor::decode::read(r)
            }
        }
    }
}

pub fn gen_try_read_cbor(ast: &SchemaType) -> TokenStream {
    let (ident, body) = match ast {
        SchemaType::Struct(s) => (&s.name, gen_try_read_cbor_struct(&s)),
        SchemaType::Union(u) => (&u.name, gen_try_read_cbor_union(&u)),
    };
    quote! {
        impl libipld::cbor::decode::TryReadCbor for #ident {
            fn try_read_cbor<R: std::io::Read>(
                r: &mut R,
                major: u8,
            ) -> libipld::Result<Option<Self>> {
                use libipld::cbor::decode::{read_key, read_len, read_u8, TryReadCbor};
                use libipld::cbor::error::LengthOutOfRange;
                use libipld::codec::Decode;
                use libipld::error::{Result, TypeError, TypeErrorType};
                let c = DagCborCodec;
                #body
            }
        }
    }
}

fn rename(name: &syn::Member, rename: Option<&String>) -> TokenStream {
    if let Some(rename) = rename {
        quote!(#rename)
    } else {
        let name = match name {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(index) => index.index.to_string(),
        };
        quote!(#name)
    }
}

fn default(binding: &syn::Ident, default: Option<&syn::Expr>, tokens: TokenStream) -> TokenStream {
    if let Some(default) = default {
        quote! {
            if #binding != &#default {
                #tokens
            }
        }
    } else {
        tokens
    }
}

fn gen_encode_match(arms: impl Iterator<Item = TokenStream>) -> TokenStream {
    quote! {
        match *self {
            #(#arms,)*
        }
        Ok(())
    }
}

fn try_read_cbor(ty: TokenStream) -> TokenStream {
    quote! {{
        if let Some(t) = #ty::try_read_cbor(r, major)? {
            t
        } else {
            return Ok(None);
        }
    }}
}

fn gen_encode_struct(s: &Struct) -> TokenStream {
    let pat = &*s.pat;
    let body = gen_encode_struct_body(s);
    gen_encode_match(std::iter::once(quote!(#pat => { #body })))
}

fn gen_encode_struct_body(s: &Struct) -> TokenStream {
    let len = s.fields.len() as u64;
    match s.repr {
        StructRepr::Map => {
            let fields = s.fields.iter().map(|field| {
                let key = rename(&field.name, field.rename.as_ref());
                let binding = &field.binding;
                default(
                    binding,
                    field.default.as_ref(),
                    quote! {
                        Encode::encode(#key, c, w)?;
                        Encode::encode(#binding, c, w)?;
                    },
                )
            });
            quote! {
                write_u64(w, 5, #len)?;
                #(#fields)*
            }
        }
        StructRepr::Tuple => {
            let fields = s.fields.iter().map(|field| {
                let binding = &field.binding;
                default(
                    binding,
                    field.default.as_ref(),
                    quote! {
                        Encode::encode(#binding, c, w)?;
                    },
                )
            });
            quote! {
                write_u64(w, 4, #len)?;
                #(#fields)*
            }
        }
        StructRepr::Value => {
            assert_eq!(s.fields.len(), 1);
            let field = &s.fields[0];
            let binding = &field.binding;
            default(
                binding,
                field.default.as_ref(),
                quote! {
                    Encode::encode(#binding, c, w)?;
                },
            )
        }
        StructRepr::Null => {
            assert_eq!(s.fields.len(), 0);
            quote!(write_null(w)?;)
        }
    }
}

fn gen_encode_union(u: &Union) -> TokenStream {
    let arms = u
        .variants
        .iter()
        .map(|s| {
            let pat = &*s.pat;
            let key = rename(&syn::Member::Named(s.name.clone()), s.rename.as_ref());
            let value = gen_encode_struct_body(s);
            match u.repr {
                UnionRepr::Keyed => {
                    quote! {
                        #pat => {
                            write_u64(w, 5, 1)?;
                            Encode::encode(#key, c, w)?;
                            #value
                        }
                    }
                }
                UnionRepr::Kinded => {
                    quote!(#pat => { #value })
                }
                UnionRepr::String => {
                    assert_eq!(s.repr, StructRepr::Null);
                    quote!(#pat => Encode::encode(#key, c, w)?)
                }
                UnionRepr::Int => {
                    assert_eq!(s.repr, StructRepr::Null);
                    quote!()
                }
            }
        })
        .collect::<Vec<_>>();
    if u.repr == UnionRepr::Int {
        quote!(Encode::encode(&(*self as u64), c, w))
    } else {
        gen_encode_match(arms.into_iter())
    }
}

fn gen_try_read_cbor_struct(s: &Struct) -> TokenStream {
    let len = s.fields.len();
    let construct = &*s.construct;
    match s.repr {
        StructRepr::Map => {
            let fields = s.fields.iter().map(|field| {
                let key = rename(&field.name, field.rename.as_ref());
                let binding = &field.binding;
                quote! {
                    read_key(r, #key)?;
                    let #binding = Decode::decode(c, r)?;
                }
            });
            quote! {
                match major {
                    0xa0..=0xbb => {
                        let len = read_len(r, major - 0xa0)?;
                        if len != #len {
                            return Err(LengthOutOfRange.into());
                        }
                        #(#fields)*
                        return Ok(Some(#construct));
                    }
                    _ => Ok(None),
                }
            }
        }
        StructRepr::Tuple => {
            let fields = s.fields.iter().map(|field| {
                let binding = &field.binding;
                quote! {
                    let #binding = Decode::decode(c, r)?;
                }
            });
            quote! {
                match major {
                    0x80..=0x9b => {
                        let len = read_len(r, major - 0x80)?;
                        if len != #len {
                            return Err(LengthOutOfRange.into());
                        }
                        #(#fields)*
                        return Ok(Some(#construct));
                    }
                    _ => Ok(None),
                }
            }
        }
        StructRepr::Value => {
            assert_eq!(s.fields.len(), 1);
            let binding = &s.fields[0].binding;
            quote! {
                if let Some(#binding) = TryReadCbor::try_read_cbor(r, major)? {
                    return Ok(Some(#construct));
                } else {
                    Ok(None)
                }
            }
        }
        StructRepr::Null => {
            assert_eq!(s.fields.len(), 0);
            quote! {
                match major {
                    0xf6..=0xf7 => {
                        return Ok(Some(#construct));
                    }
                    _ => Ok(None),
                }
            }
        }
    }
}

fn gen_try_read_cbor_union(u: &Union) -> TokenStream {
    match u.repr {
        UnionRepr::Keyed => {
            let variants = u.variants.iter().map(|s| {
                let key = rename(&syn::Member::Named(s.name.clone()), s.rename.as_ref());
                let parse = gen_try_read_cbor_struct(s);
                quote! {
                    if key.as_str() == #key {
                        let major = read_u8(r)?;
                        let res: Result<Option<Self>> = #parse;
                        res?;
                    }
                }
            });
            quote! {
                if major != 0xa1 {
                    return Ok(None);
                }
                let key: String = Decode::decode(c, r)?;
                #(#variants;)*
                Err(TypeError::new(TypeErrorType::Key(key), TypeErrorType::Null).into())
            }
        }
        UnionRepr::Kinded => {
            let variants = u.variants.iter().map(|s| {
                let parse = gen_try_read_cbor_struct(s);
                quote! {
                    let res: Result<Option<Self>> = #parse;
                    res?;
                }
            });
            quote! {
                #(#variants;)*
                Err(TypeError::new(TypeErrorType::Null, TypeErrorType::Null).into())
            }
        }
        UnionRepr::String => {
            let arms = u.variants.iter().map(|v| {
                let pat = &*v.pat;
                let value = rename(&syn::Member::Named(v.name.clone()), v.rename.as_ref());
                quote!(#value => #pat)
            });
            let parse = try_read_cbor(quote!(String));
            quote! {
                let key = #parse;
                let res = match key.as_str() {
                    #(#arms,)*
                    _ => return Err(TypeError::new(TypeErrorType::Key(key.to_string()), TypeErrorType::Null).into()),
                };
                Ok(Some(res))
            }
        }
        UnionRepr::Int => {
            let arms = u.variants.iter().map(|v| {
                let pat = &*v.pat;
                quote!(x if x == #pat as u64 => #pat)
            });
            let parse = try_read_cbor(quote!(u64));
            quote! {
                let key = #parse;
                let res = match key {
                    #(#arms,)*
                    _ => return Err(TypeError::new(TypeErrorType::Key(key.to_string()), TypeErrorType::Null).into()),
                };
                Ok(Some(res))
            }
        }
    }
}
