import { TypeTable } from "fumadocs-ui/components/type-table";
import type { TypeNode } from "fumadocs-ui/components/type-table";

type PropRow = [
  name: string,
  type: string,
  defaultValue: string | null,
  description: string,
];

interface PropsTableProps {
  rows: PropRow[];
}

const toTypeNode = ([
  ,
  type,
  defaultValue,
  description,
]: PropRow): TypeNode => ({
  default: defaultValue ?? undefined,
  description,
  type,
});

export const PropsTable = ({ rows }: PropsTableProps) => (
  <TypeTable
    type={Object.fromEntries(rows.map((row) => [row[0], toTypeNode(row)]))}
  />
);
