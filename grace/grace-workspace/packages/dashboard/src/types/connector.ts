export type ConnectorStatus =
  | "supported"
  | "not_supported"
  | "not_implemented"
  | "error";

export interface ConnectorPaymentMethod {
  category: string;
  method: string;
  status: ConnectorStatus;
}

export interface ConnectorFlow {
  name: string;
  status: ConnectorStatus;
}

export interface ConnectorStats {
  total: number;
  supported: number;
  notImplemented: number;
  notSupported: number;
  error: number;
}

export interface Connector {
  name: string;
  filePath: string;
  paymentMethods: ConnectorPaymentMethod[];
  stats: ConnectorStats;
  flows: ConnectorFlow[];
  flowStats: ConnectorStats;
}

export type NotImplementedItem =
  | {
      kind: "method";
      connector: string;
      category: string;
      method: string;
      filePath: string;
    }
  | {
      kind: "flow";
      connector: string;
      flow: string;
      filePath: string;
    };

export type MethodGap = Extract<NotImplementedItem, { kind: "method" }>;
export type FlowGap = Extract<NotImplementedItem, { kind: "flow" }>;
