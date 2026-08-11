//!
//! Formatting [layer](https://docs.rs/tracing-subscriber/0.3.15/tracing_subscriber/layer/trait.Layer.html) for Router.
//!

use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::Write,
};

use common_utils::consts::{
    LOG_FILE as FILE, LOG_FN as FN, LOG_FULL_NAME as FULL_NAME, LOG_HOSTNAME as HOSTNAME,
    LOG_LEVEL as LEVEL, LOG_LINE as LINE, LOG_MESSAGE as MESSAGE, LOG_PID as PID,
    LOG_SERVICE as SERVICE, LOG_TARGET as TARGET, LOG_TIME as TIME,
};
use once_cell::sync::Lazy;
use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;

use super::storage::Storage;
use time::format_description::well_known::Iso8601;
use tracing::{Event, Metadata, Subscriber};
use tracing_subscriber::{
    fmt::MakeWriter,
    layer::Context,
    registry::{LookupSpan, SpanRef},
    Layer,
};

// TODO: Documentation coverage for this crate

// Implicit keys

/// Set of predefined implicit keys.
pub static IMPLICIT_KEYS: Lazy<rustc_hash::FxHashSet<&str>> = Lazy::new(|| {
    let mut set = rustc_hash::FxHashSet::default();

    set.insert(HOSTNAME);
    set.insert(PID);
    set.insert(LEVEL);
    set.insert(TARGET);
    set.insert(SERVICE);
    set.insert(LINE);
    set.insert(FILE);
    set.insert(FN);
    set.insert(FULL_NAME);
    set.insert(TIME);

    set
});

/// Describe type of record: entering a span, exiting a span, an event.
#[derive(Clone, Debug)]
pub enum RecordType {
    /// Entering a span.
    EnterSpan,
    /// Exiting a span.
    ExitSpan,
    /// Event.
    Event,
}

impl fmt::Display for RecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Self::EnterSpan => "START",
            Self::ExitSpan => "END",
            Self::Event => "EVENT",
        };
        write!(f, "{repr}")
    }
}

///
/// Format log records.
/// `FormattingLayer` relies on the `tracing_bunyan_formatter::JsonStorageLayer` which is storage of entries.
///
#[derive(Debug)]
pub struct FormattingLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    dst_writer: W,
    pid: u32,
    hostname: String,
    service: String,
    default_fields: HashMap<String, Value>,
}

impl<W> FormattingLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    ///
    /// Constructor of `FormattingLayer`.
    ///
    /// A `name` will be attached to all records during formatting.
    /// A `dst_writer` to forward all records.
    ///
    /// ## Example
    /// ```rust,ignore
    /// let formatting_layer = router_env::FormattingLayer::new(env::service_name!(),std::io::stdout);
    /// ```
    ///
    pub fn new(service: &str, dst_writer: W) -> Self {
        Self::new_with_implicit_entries(service, dst_writer, HashMap::new())
    }

    /// Construct of `FormattingLayer with implicit default entries.
    pub fn new_with_implicit_entries(
        service: &str,
        dst_writer: W,
        mut default_fields: HashMap<String, Value>,
    ) -> Self {
        let pid = std::process::id();
        let hostname = gethostname::gethostname().to_string_lossy().into_owned();
        let service = service.to_string();
        default_fields.retain(|key, value| {
            if !IMPLICIT_KEYS.contains(key.as_str()) {
                true
            } else {
                #[allow(clippy::print_stderr)]
                {
                    eprintln!(
                        "Attempting to log a reserved entry. It won't be added to the logs. key: {:?}, value: {:?}",
                        key, value);
                }
                false
            }
        });

        Self {
            dst_writer,
            pid,
            hostname,
            service,
            default_fields,
        }
    }

    /// Serialize common for both span and event entries.
    fn common_serialize<S>(
        &self,
        map_serializer: &mut impl SerializeMap<Error = serde_json::Error>,
        metadata: &Metadata<'_>,
        span: Option<&SpanRef<'_, S>>,
        storage: &Storage<'_>,
        name: &str,
    ) -> Result<(), std::io::Error>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let is_extra = |s: &str| !IMPLICIT_KEYS.contains(s);

        map_serializer.serialize_entry(HOSTNAME, &self.hostname)?;
        map_serializer.serialize_entry(PID, &self.pid)?;
        map_serializer.serialize_entry(LEVEL, &format_args!("{}", metadata.level()))?;
        map_serializer.serialize_entry(TARGET, metadata.target())?;
        map_serializer.serialize_entry(SERVICE, &self.service)?;
        map_serializer.serialize_entry(LINE, &metadata.line())?;
        map_serializer.serialize_entry(FILE, &metadata.file())?;
        map_serializer.serialize_entry(FN, name)?;
        map_serializer
            .serialize_entry(FULL_NAME, &format_args!("{}::{}", metadata.target(), name))?;
        if let Ok(time) = &time::OffsetDateTime::now_utc().format(&Iso8601::DEFAULT) {
            map_serializer.serialize_entry(TIME, time)?;
        }

        // Write down implicit default entries.
        for (key, value) in self.default_fields.iter() {
            map_serializer.serialize_entry(key, value)?;
        }

        let mut explicit_entries_set: HashSet<&str> = HashSet::default();
        // Write down explicit event's entries.

        for (key, value) in storage.values.iter() {
            map_serializer.serialize_entry(key, value)?;
            explicit_entries_set.insert(key);
        }

        // Write down entries from the span, if it exists.
        if let Some(span) = &span {
            let extensions = span.extensions();
            if let Some(visitor) = extensions.get::<Storage<'_>>() {
                for (key, value) in &visitor.values {
                    if is_extra(key) && !explicit_entries_set.contains(key) {
                        map_serializer.serialize_entry(key, value)?;
                    } else {
                        #[allow(clippy::print_stderr)]
                        {
                            eprintln!(
                                "Attempting to log a reserved entry. It won't be added to the logs. key: {key:?}, value: {value:?}"
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    ///
    /// Flush memory buffer into an output stream trailing it with next line.
    ///
    /// Should be done by single `write_all` call to avoid fragmentation of log because of multithreading.
    ///
    fn flush(&self, mut buffer: Vec<u8>) -> Result<(), std::io::Error> {
        buffer.write_all(b"\n")?;
        self.dst_writer.make_writer().write_all(&buffer)
    }

    /// Serialize entries of span.
    fn span_serialize<S>(
        &self,
        span: &SpanRef<'_, S>,
        ty: RecordType,
    ) -> Result<Vec<u8>, std::io::Error>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut map_serializer = serializer.serialize_map(None)?;
        let message = Self::span_message(span, ty);
        let mut storage = Storage::default();
        storage.record_value(MESSAGE, message.into());

        self.common_serialize(
            &mut map_serializer,
            span.metadata(),
            Some(span),
            &storage,
            span.name(),
        )?;

        map_serializer.end()?;
        Ok(buffer)
    }

    /// Serialize event into a buffer of bytes using parent span.
    pub fn event_serialize<S>(
        &self,
        span: &Option<&SpanRef<'_, S>>,
        event: &Event<'_>,
    ) -> std::io::Result<Vec<u8>>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut map_serializer = serializer.serialize_map(None)?;

        let mut storage = Storage::default();
        event.record(&mut storage);

        let name = span.map_or("?", SpanRef::name);
        Self::event_message(span, event, &mut storage);

        self.common_serialize(&mut map_serializer, event.metadata(), *span, &storage, name)?;

        map_serializer.end()?;
        Ok(buffer)
    }

    ///
    /// Format message of a span.
    ///
    /// Example: "[FN_WITHOUT_COLON - START]"
    ///
    fn span_message<S>(span: &SpanRef<'_, S>, ty: RecordType) -> String
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        format!("[{} - {}]", span.metadata().name().to_uppercase(), ty)
    }

    ///
    /// Format message of an event.
    ///
    /// Examples: "[FN_WITHOUT_COLON - EVENT] Message"
    ///
    fn event_message<S>(
        span: &Option<&SpanRef<'_, S>>,
        event: &Event<'_>,
        storage: &mut Storage<'_>,
    ) where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let message = storage
            .values
            .entry(MESSAGE)
            .or_insert_with(|| event.metadata().target().into());

        // Prepend the span name to the message if span exists.
        if let (Some(span), Value::String(a)) = (span, message) {
            *a = format!("{} {}", Self::span_message(span, RecordType::Event), a,);
        }
    }
}

#[allow(clippy::expect_used)]
impl<S, W> Layer<S> for FormattingLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Event could have no span.
        let span = ctx.lookup_current();

        let result: std::io::Result<Vec<u8>> = self.event_serialize(&span.as_ref(), event);
        if let Ok(formatted) = result {
            let _ = self.flush(formatted);
        }
    }

    fn on_enter(&self, id: &tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("No span");
        if let Ok(serialized) = self.span_serialize(&span, RecordType::EnterSpan) {
            let _ = self.flush(serialized);
        }
    }

    fn on_close(&self, id: tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("No span");
        if let Ok(serialized) = self.span_serialize(&span, RecordType::ExitSpan) {
            let _ = self.flush(serialized);
        }
    }
}
