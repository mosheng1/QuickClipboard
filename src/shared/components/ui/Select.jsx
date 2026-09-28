function Select({
  value,
  onChange,
  options,
  className = '',
  disabled = false
}) {
  const selectedOption = options.find(option => String(option.value) === String(value)) ?? options[0];

  return <span className={`qc-select ${disabled ? 'qc-select-disabled' : ''} ${className}`}>
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        disabled={disabled}
        className="qc-select-native px-3 py-2 bg-qc-panel border border-qc-border rounded-lg text-qc-fg focus:outline-none focus:ring-2 focus:ring-blue-500 cursor-pointer disabled:cursor-not-allowed disabled:opacity-60"
      >
        {options.map(option => <option key={option.value} value={option.value}>
            {option.label}
          </option>)}
      </select>
      <span className="qc-select-display" aria-hidden="true">
        {selectedOption?.label}
      </span>
    </span>;
}
export default Select;
