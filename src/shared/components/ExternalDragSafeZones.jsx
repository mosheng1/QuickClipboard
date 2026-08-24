export default function ExternalDragSafeZones({ visible }) {
  return (
    <div className={`fixed inset-[5px] z-[1100] overflow-hidden rounded-[8px] pointer-events-none transition-opacity duration-200 ease-out ${visible ? 'opacity-100' : 'opacity-0'}`} aria-hidden="true">
      <div className="absolute inset-y-0 left-0 w-3 border-r border-dashed border-qc-border-strong bg-qc-active/70 flex items-center justify-center">
        <span className="text-[10px] font-medium tracking-[0.12em] text-qc-fg" style={{ writingMode: 'vertical-rl' }}>拖出外部</span>
      </div>
      <div className="absolute inset-y-0 right-0 w-3 border-l border-dashed border-qc-border-strong bg-qc-active/70 flex items-center justify-center">
        <span className="text-[10px] font-medium tracking-[0.12em] text-qc-fg" style={{ writingMode: 'vertical-rl' }}>拖出外部</span>
      </div>
    </div>
  );
}
