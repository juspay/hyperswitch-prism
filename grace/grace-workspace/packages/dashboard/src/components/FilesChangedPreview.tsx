import { T } from "../theme";
import type { FileChangePreview } from "../../../core/src/types";

interface Props {
  files: FileChangePreview[];
}

export function FilesChangedPreview({ files }: Props) {
  const totalAdditions = files.reduce((sum, f) => sum + f.linesAdded, 0);
  const totalDeletions = files.reduce((sum, f) => sum + f.linesRemoved, 0);

  const getChangeIcon = (changeType: string) => {
    switch (changeType) {
      case "created":
        return "+";
      case "deleted":
        return "−";
      default:
        return "•";
    }
  };

  const getChangeColor = (changeType: string) => {
    switch (changeType) {
      case "created":
        return T.success;
      case "deleted":
        return T.error;
      default:
        return T.warn;
    }
  };

  const getChangeBg = (changeType: string) => {
    switch (changeType) {
      case "created":
        return T.successSoft;
      case "deleted":
        return T.errorSoft;
      default:
        return T.warnSoft;
    }
  };

  return (
    <div
      style={{
        border: `1px solid ${T.border}`,
        borderRadius: 8,
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: "12px 16px",
          background: T.bgElev,
          borderBottom: `1px solid ${T.border}`,
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span style={{ fontWeight: 600, fontSize: 14 }}>
          {files.length} file{files.length !== 1 ? "s" : ""} will be changed
        </span>
        <span style={{ color: T.textMuted, fontSize: 13 }}>
          <span style={{ color: T.success }}>+{totalAdditions}</span>
          {" "}
          <span style={{ color: T.error }}>-{totalDeletions}</span>
        </span>
      </div>

      {/* File list */}
      {files.map((file, idx) => (
        <div
          key={file.path}
          style={{
            padding: "12px 16px",
            borderBottom:
              idx < files.length - 1 ? `1px solid ${T.border}` : undefined,
            display: "flex",
            alignItems: "flex-start",
            gap: 12,
            background: T.bg,
          }}
        >
          {/* Change type icon */}
          <div
            style={{
              width: 20,
              height: 20,
              borderRadius: "50%",
              background: getChangeBg(file.changeType),
              color: getChangeColor(file.changeType),
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 12,
              fontWeight: 700,
              flexShrink: 0,
              marginTop: 2,
            }}
          >
            {getChangeIcon(file.changeType)}
          </div>

          {/* File info */}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div
              style={{
                fontFamily:
                  'ui-monospace, SFMono-Regular, Consolas, monospace',
                fontSize: 13,
                color: T.text,
                marginBottom: 4,
                wordBreak: "break-all",
              }}
            >
              {file.path}
            </div>
            <div
              style={{
                fontSize: 12,
                color: T.textMuted,
                lineHeight: 1.5,
              }}
            >
              {file.description}
            </div>
            {file.previewSnippet && (
              <pre
                style={{
                  marginTop: 8,
                  padding: 10,
                  background: T.codeBg,
                  borderRadius: 4,
                  fontSize: 11,
                  overflow: "auto",
                  border: `1px solid ${T.border}`,
                  fontFamily:
                    'ui-monospace, SFMono-Regular, monospace',
                }}
              >
                <code>{file.previewSnippet}</code>
              </pre>
            )}
          </div>

          {/* Line stats */}
          <div
            style={{
              display: "flex",
              gap: 8,
              fontSize: 12,
              fontFamily: "ui-monospace, monospace",
              flexShrink: 0,
            }}
          >
            {file.linesAdded > 0 && (
              <span style={{ color: T.success }}>+{file.linesAdded}</span>
            )}
            {file.linesRemoved > 0 && (
              <span style={{ color: T.error }}>-{file.linesRemoved}</span>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
