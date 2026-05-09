import { useState } from "react";
import type { M3u8Option } from "../lib/types";

interface Props {
  options: M3u8Option[];
  onConfirm: (option: M3u8Option) => void;
  onCancel: () => void;
}

export function ResolutionDialog({ options, onConfirm, onCancel }: Props) {
  const [selectedUrl, setSelectedUrl] = useState(options[0]?.url ?? "");
  const selected = options.find((option) => option.url === selectedUrl) ?? options[0];

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="modal" role="dialog" aria-modal="true" aria-labelledby="resolution-title">
        <h2 id="resolution-title">選擇解析度</h2>
        <div className="resolution-list">
          {options.map((option) => (
            <label className="resolution-option" key={option.url}>
              <input
                type="radio"
                name="resolution"
                checked={selectedUrl === option.url}
                onChange={() => setSelectedUrl(option.url)}
              />
              <span>{option.resolution}</span>
              <small>{option.url}</small>
            </label>
          ))}
        </div>
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>取消</button>
          <button className="primary-button" type="button" onClick={() => selected && onConfirm(selected)}>確認</button>
        </div>
      </section>
    </div>
  );
}
