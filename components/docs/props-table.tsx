import { TypeTable } from "fumadocs-ui/components/type-table";
import type { TypeNode } from "fumadocs-ui/components/type-table";

type PropRow = [
  name: string,
  type: string,
  defaultValue: string | null,
  description: string | null,
];

interface PropsTableProps {
  rows: PropRow[];
}

const toTypeNode = (row: PropRow): TypeNode => ({
  default: row[2] ?? undefined,
  description: row[3] ?? undefined,
  type: row[1],
});

export const PropsTable = ({ rows }: PropsTableProps) => {
  if (rows.length === 0) {
    return <p className="text-muted-foreground text-sm">No props.</p>;
  }
  return (
    <TypeTable
      type={Object.fromEntries(rows.map((row) => [row[0], toTypeNode(row)]))}
    />
  );
};
