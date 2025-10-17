use std::path::Path;

#[derive(Default)]
pub struct ExportData {}
impl ExportData {
	pub fn calculate(&mut self) {}

	pub fn save_snapshot_csv<P: AsRef<Path>>(&self, path: P, data: &crate::data::traces::TracesData) -> std::io::Result<()> {
		use std::io::Write;
		let mut f = std::fs::File::create(path)?;
		// header
		writeln!(f, "trace,timestamp,value")?;
		for name in &data.trace_order {
			if let Some(tr) = data.traces.get(name) {
				for p in tr.live.iter() {
					writeln!(f, "{},{:.9},{}", name, p[0], p[1] + tr.offset)?;
				}
			}
		}
		Ok(())
	}

	#[cfg(feature = "parquet")]
	pub fn save_snapshot_parquet<P: AsRef<Path>>(&self, path: P, data: &crate::data::traces::TracesData) -> parquet::errors::Result<()> {
		use arrow_array::{Float64Array, StringArray, RecordBatch};
		use arrow_schema::{Schema, Field, DataType};
		use parquet::arrow::arrow_writer::ArrowWriter;
		use parquet::file::properties::WriterProperties;

		let mut trace_col: Vec<String> = Vec::new();
		let mut ts_col: Vec<f64> = Vec::new();
		let mut val_col: Vec<f64> = Vec::new();
		for name in &data.trace_order {
			if let Some(tr) = data.traces.get(name) {
				for p in tr.live.iter() { trace_col.push(name.clone()); ts_col.push(p[0]); val_col.push(p[1] + tr.offset); }
			}
		}
		let schema = Schema::new(vec![
			Field::new("trace", DataType::Utf8, false),
			Field::new("timestamp", DataType::Float64, false),
			Field::new("value", DataType::Float64, false),
		]);
		let batch = RecordBatch::try_new(
			std::sync::Arc::new(schema.clone()),
			vec![
				std::sync::Arc::new(StringArray::from(trace_col)) as _,
				std::sync::Arc::new(Float64Array::from(ts_col)) as _,
				std::sync::Arc::new(Float64Array::from(val_col)) as _,
			],
		).unwrap();
		let file = std::fs::File::create(path)?;
		let props = WriterProperties::builder().build();
		let mut writer = ArrowWriter::try_new(file, std::sync::Arc::new(schema), Some(props))?;
		writer.write(&batch)?;
		writer.close()?;
		Ok(())
	}
}
