use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;
use sqlx::postgres::{PgRow, PgTypeInfo, PgValueRef};
use sqlx::{Column, Decode, Row, TypeInfo, ValueRef};
use std::borrow::Cow;

use serde::de::value::SeqDeserializer;
use sqlx::error::BoxDynError;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

macro_rules! delegate_to_deserialize_any {
    ($($fn_name:ident), *) => {
        $(
            fn $fn_name<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
        )*
    };
}

pub struct DbRow(pub PgRow);

impl<'de> IntoDeserializer<'de, Error> for DbRow {
    type Deserializer = DbRow;
    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> Deserializer<'de> for DbRow {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    delegate_to_deserialize_any! {
        deserialize_bool, deserialize_char,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64,

        deserialize_f32, deserialize_f64, deserialize_str, deserialize_string,
        deserialize_unit, deserialize_bytes, deserialize_byte_buf, deserialize_identifier
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(MapSeqqDeserializer {
            index: 0,
            inner: &self,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if len != self.0.len() {
            return Err(Error::custom(""));
        }
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if len != self.0.len() {
            return Err(Error::custom(""));
        }
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(MapSeqqDeserializer {
            index: 0,
            inner: &self,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(MapSeqqDeserializer {
            index: 0,
            inner: &self,
        })
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_none()
    }
}

macro_rules! delegate_decode {
    ($($fn_name:ident|$visit_method:ident),*) => {
        $(
            fn $fn_name<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                visitor.$visit_method(Decode::decode(self.column).map_err(Error::DecodeError)?)
            }
        )*
    };
}

pub struct DbColumn<'a> {
    column: PgValueRef<'a>,
}

impl<'de: 'a, 'a> Deserializer<'de> for DbColumn<'a> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // determine type information here: TODO
        let kind = match self.column.type_info() {
            Cow::Borrowed(ty) => Cow::Borrowed(ty.name()),
            Cow::Owned(ty) => Cow::Owned(ty.to_string()),
        };
        match kind.as_ref() {
            "INT8" => self.deserialize_i64(visitor),
            "INT4" => self.deserialize_i32(visitor),
            "INT2" => self.deserialize_i16(visitor),
            "TEXT" | "VARCHAR" => self.deserialize_str(visitor),
            "BOOL" => self.deserialize_bool(visitor),
            _ => {
                unimplemented!()
            }
        }
    }

    delegate_decode! {
        deserialize_bool|visit_bool,
        deserialize_i8|visit_i8, deserialize_i16|visit_i16, deserialize_i32|visit_i32, deserialize_i64|visit_i64,
        deserialize_u8|visit_i8, deserialize_u16|visit_i16, deserialize_u32|visit_i32, deserialize_u64|visit_i64,
        deserialize_f32|visit_f32, deserialize_f64|visit_f64, deserialize_str|visit_str, deserialize_string|visit_string,

        deserialize_bytes|visit_bytes, deserialize_byte_buf|visit_byte_buf

    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let value: i8 = Decode::decode(self.column).map_err(Error::DecodeError)?;
        visitor.visit_char(u8::try_from(value).unwrap().into())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.column.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.column.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_unit()
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
}

pub struct MapSeqqDeserializer<'a> {
    inner: &'a DbRow,
    index: usize,
}

impl<'de: 'a, 'a> SeqAccess<'de> for MapSeqqDeserializer<'a> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.index >= self.inner.0.len() {
            return Ok(None);
        }
        let column = self.inner.0.try_get_raw(self.index)?;
        self.index += 1;
        let column_deserializer = DbColumn { column };

        T::deserialize(seed, column_deserializer).map(Some)
    }
}

impl<'de: 'a, 'a> MapAccess<'de> for MapSeqqDeserializer<'a> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.index >= self.inner.0.len() {
            return Ok(None);
        }

        let column = self.inner.0.column(self.index);
        let column_name = column.name();
        seed.deserialize(column_name.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let column = self.inner.0.try_get_raw(self.index)?;
        self.index += 1;
        seed.deserialize(DbColumn { column })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("DB Deserialization error: {0}")]
    Custom(String),
    #[error("A decode error occurred: {0}")]
    DecodeError(BoxDynError),

    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

impl Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        serde::de::Error::custom(msg)
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Custom(msg.to_string())
    }
}
