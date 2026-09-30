"use client";

import type { ReactNode } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

interface ComponentPreviewTabsProps {
  align?: "center" | "start" | "end";
  className?: string;
  code: ReactNode;
  preview: ReactNode;
}

export const ComponentPreviewTabs = ({
  align = "center",
  className,
  code,
  preview,
}: ComponentPreviewTabsProps) => (
  <Tabs className="not-prose my-6 gap-3" defaultValue="preview">
    <TabsList variant="line">
      <TabsTrigger value="preview">Preview</TabsTrigger>
      <TabsTrigger value="code">Code</TabsTrigger>
    </TabsList>
    <TabsContent value="preview">
      <div
        className={cn(
          "bg-background flex min-h-72 w-full justify-center rounded-xl border p-6 sm:p-10",
          align === "center" && "items-center",
          align === "start" && "items-start",
          align === "end" && "items-end",
          className
        )}
        data-slot="component-preview"
      >
        {preview}
      </div>
    </TabsContent>
    <TabsContent className="[&_figure]:my-0 [&_pre]:max-h-[32rem]" value="code">
      {code}
    </TabsContent>
  </Tabs>
);
