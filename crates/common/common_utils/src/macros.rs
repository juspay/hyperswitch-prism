mod id_type {
    /// Defines an ID type.
    #[macro_export]
    macro_rules! id_type {
        ($type:ident, $doc:literal, $max_length:expr, $min_length:expr) => {
            #[doc = $doc]
            #[derive(
                Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
            )]
            #[schema(value_type = String)]
            pub struct $type($crate::id_type::LengthId<$max_length, $min_length>);
        };
        ($type:ident, $doc:literal) => {
            $crate::id_type!(
                $type,
                $doc,
                { $crate::consts::MAX_ALLOWED_MERCHANT_REFERENCE_ID_LENGTH },
                { $crate::consts::MIN_REQUIRED_MERCHANT_REFERENCE_ID_LENGTH }
            );
        };
    }

    /// Defines a Global Id type
    #[macro_export]
    macro_rules! global_id_type {
        ($type:ident, $doc:literal) => {
            #[doc = $doc]
            #[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]

            pub struct $type($crate::global_id::GlobalId);
        };
    }

    /// Implements common methods on the specified ID type.
    #[macro_export]
    macro_rules! impl_id_type_methods {
        ($type:ty, $field_name:literal) => {
            impl $type {
                /// Get the string representation of the ID type.
                pub fn get_string_repr(&self) -> &str {
                    &self.0 .0 .0
                }
            }
        };
    }

    /// Implements the `Debug` trait on the specified ID type.
    #[macro_export]
    macro_rules! impl_debug_id_type {
        ($type:ty) => {
            impl core::fmt::Debug for $type {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.debug_tuple(stringify!($type))
                        .field(&self.0 .0 .0)
                        .finish()
                }
            }
        };
    }

    /// Implements the `TryFrom<Cow<'static, str>>` trait on the specified ID type.
    #[macro_export]
    macro_rules! impl_try_from_cow_str_id_type {
        ($type:ty, $field_name:literal) => {
            impl TryFrom<std::borrow::Cow<'static, str>> for $type {
                type Error = error_stack::Report<$crate::errors::ValidationError>;

                fn try_from(value: std::borrow::Cow<'static, str>) -> Result<Self, Self::Error> {
                    use error_stack::ResultExt;

                    let merchant_ref_id = $crate::id_type::LengthId::from(value).change_context(
                        $crate::errors::ValidationError::IncorrectValueProvided {
                            field_name: $field_name,
                        },
                    )?;

                    Ok(Self(merchant_ref_id))
                }
            }
        };
    }

    /// Implements the `Default` trait on the specified ID type.
    #[macro_export]
    macro_rules! impl_default_id_type {
        ($type:ty, $prefix:literal) => {
            impl Default for $type {
                fn default() -> Self {
                    Self($crate::generate_ref_id_with_default_length($prefix))
                }
            }
        };
    }

    /// Implements the `GenerateId` trait on the specified ID type.
    #[macro_export]
    macro_rules! impl_generate_id_id_type {
        ($type:ty, $prefix:literal) => {
            impl $crate::id_type::GenerateId for $type {
                fn generate() -> Self {
                    Self($crate::generate_ref_id_with_default_length($prefix))
                }
            }
        };
    }

    /// Implements the `SerializableSecret` trait on the specified ID type.
    #[macro_export]
    macro_rules! impl_serializable_secret_id_type {
        ($type:ty) => {
            impl hyperswitch_masking::SerializableSecret for $type {}
        };
    }
}

/// Collects names of all optional fields that are `None`.
/// This is typically useful for constructing error messages including a list of all missing fields.
#[macro_export]
macro_rules! collect_missing_value_keys {
    [$(($key:literal, $option:expr)),+] => {
        {
            let mut keys: Vec<&'static str> = Vec::new();
            $(
                if $option.is_none() {
                    keys.push($key);
                }
            )*
            keys
        }
    };
}
