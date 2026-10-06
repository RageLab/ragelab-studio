import type {
  AssetOperationCapability,
  OperationSpec,
} from "$lib/native";

export type EditableOperationId =
  | "ydr.translate"
  | "ydr.rebind-texture"
  | "ydr.rebind-shader"
  | "ydd.translate"
  | "ydd.rebind-texture"
  | "ydd.rebind-shader"
  | "ytd.replace-dds"
  | "ytd.repack-dds"
  | "ytd.repack-png"
  | "ytd.repack-rgba"
  | "ytd.rebuild-compact"
  | "ybn.edit-polygon";

export type OperationFieldKind = "integer" | "number" | "select" | "file";

export interface OperationFieldDefinition {
  key: string;
  label: string;
  kind: OperationFieldKind;
  required: boolean;
  min?: number;
  step?: number;
  options?: string[];
  placeholder?: string;
}

export interface OperationFormDefinition {
  id: EditableOperationId;
  label: string;
  description: string;
  fields: OperationFieldDefinition[];
}

const FORM_DEFINITIONS: Record<EditableOperationId, OperationFormDefinition> = {
  "ydr.translate": {
    id: "ydr.translate",
    label: "Translate YDR",
    description: "Rigidly translate the supported drawable.",
    fields: vectorDeltaFields(),
  },
  "ydr.rebind-texture": {
    id: "ydr.rebind-texture",
    label: "Rebind YDR texture",
    description:
      "Rebind an existing texture parameter to another compatible existing binding.",
    fields: textureBindingFields(false),
  },
  "ydr.rebind-shader": {
    id: "ydr.rebind-shader",
    label: "Rebind YDR shader",
    description: "Point an existing geometry at another shader already in the asset.",
    fields: shaderBindingFields(false),
  },
  "ydd.translate": {
    id: "ydd.translate",
    label: "Translate YDD drawable",
    description: "Rigidly translate one selected drawable in the dictionary.",
    fields: [
      integerField("drawableIndex", "Drawable index"),
      ...vectorDeltaFields(),
    ],
  },
  "ydd.rebind-texture": {
    id: "ydd.rebind-texture",
    label: "Rebind YDD texture",
    description:
      "Rebind a texture parameter within one selected drawable using existing bindings.",
    fields: textureBindingFields(true),
  },
  "ydd.rebind-shader": {
    id: "ydd.rebind-shader",
    label: "Rebind YDD shader",
    description:
      "Rebind one geometry inside a selected drawable to an existing shader.",
    fields: shaderBindingFields(true),
  },
  "ytd.replace-dds": {
    id: "ytd.replace-dds",
    label: "Replace YTD DDS",
    description:
      "Layout-preserving replacement using a compatible classic DDS payload.",
    fields: [
      integerField("textureIndex", "Texture index"),
      fileField("replacement", "Replacement DDS"),
    ],
  },
  "ytd.repack-dds": {
    id: "ytd.repack-dds",
    label: "Repack YTD DDS",
    description:
      "Rebuild a selected texture from a classic DDS payload, allowing relocation.",
    fields: [
      integerField("textureIndex", "Texture index"),
      fileField("replacement", "Replacement DDS"),
    ],
  },
  "ytd.repack-png": {
    id: "ytd.repack-png",
    label: "Repack YTD PNG",
    description:
      "Decode a PNG through RageLab Core, preserve the proven Legacy target format, and regenerate the complete mip chain.",
    fields: [
      integerField("textureIndex", "Texture index"),
      fileField("replacement", "Replacement PNG"),
    ],
  },
  "ytd.repack-rgba": {
    id: "ytd.repack-rgba",
    label: "Repack YTD RGBA",
    description:
      "Rebuild a selected texture from an external raw RGBA8 payload.",
    fields: [
      integerField("textureIndex", "Texture index"),
      {
        key: "width",
        label: "Width",
        kind: "integer",
        required: true,
        min: 1,
        step: 1,
      },
      {
        key: "height",
        label: "Height",
        kind: "integer",
        required: true,
        min: 1,
        step: 1,
      },
      fileField("replacement", "Raw RGBA payload"),
    ],
  },
  "ytd.rebuild-compact": {
    id: "ytd.rebuild-compact",
    label: "Rebuild compact YTD",
    description:
      "Rebuild the Legacy v13 dictionary through the deterministic compact serializer.",
    fields: [],
  },
  "ybn.edit-polygon": {
    id: "ybn.edit-polygon",
    label: "Edit YBN shape polygon",
    description:
      "Edit radius and/or material of an existing sphere, capsule, box, or cylinder polygon.",
    fields: [
      integerField("childIndex", "Child index"),
      integerField("polygonIndex", "Polygon index"),
      {
        key: "kind",
        label: "Shape kind",
        kind: "select",
        required: true,
        options: ["sphere", "capsule", "box", "cylinder"],
      },
      {
        key: "radius",
        label: "Radius (optional)",
        kind: "number",
        required: false,
        min: 0.000001,
        step: 0.01,
        placeholder: "Leave blank to preserve radius",
      },
      {
        key: "materialIndex",
        label: "Material index (optional)",
        kind: "integer",
        required: false,
        min: 0,
        step: 1,
        placeholder: "Leave blank to preserve material",
      },
    ],
  },
};

export function editableOperationCapabilities(
  capabilities: AssetOperationCapability[],
): AssetOperationCapability[] {
  return capabilities.filter(
    (capability) =>
      capability.writesAsset &&
      isEditableOperationId(capability.id) &&
      (capability.availability === "available" ||
        capability.availability === "parameterized"),
  );
}

export function operationFormDefinition(
  id: string,
): OperationFormDefinition | null {
  return isEditableOperationId(id) ? FORM_DEFINITIONS[id] : null;
}

export function defaultOperationValues(
  definition: OperationFormDefinition,
): Record<string, string> {
  return Object.fromEntries(
    definition.fields.map((field) => [
      field.key,
      field.kind === "select" ? (field.options?.[0] ?? "") : "",
    ]),
  );
}

export function buildOperationSpec(
  definition: OperationFormDefinition,
  values: Record<string, string>,
): OperationSpec {
  switch (definition.id) {
    case "ydr.translate":
      return {
        type: definition.id,
        delta: parseDelta(values),
      };
    case "ydd.translate":
      return {
        type: definition.id,
        drawableIndex: requiredInteger(values, "drawableIndex"),
        delta: parseDelta(values),
      };
    case "ydr.rebind-texture":
      return {
        type: definition.id,
        sourceShader: requiredInteger(values, "sourceShader"),
        sourceParameter: requiredInteger(values, "sourceParameter"),
        targetShader: requiredInteger(values, "targetShader"),
        targetParameter: requiredInteger(values, "targetParameter"),
      };
    case "ydd.rebind-texture":
      return {
        type: definition.id,
        drawableIndex: requiredInteger(values, "drawableIndex"),
        sourceShader: requiredInteger(values, "sourceShader"),
        sourceParameter: requiredInteger(values, "sourceParameter"),
        targetShader: requiredInteger(values, "targetShader"),
        targetParameter: requiredInteger(values, "targetParameter"),
      };
    case "ydr.rebind-shader":
      return {
        type: definition.id,
        modelIndex: requiredInteger(values, "modelIndex"),
        geometryIndex: requiredInteger(values, "geometryIndex"),
        targetShaderIndex: requiredInteger(values, "targetShaderIndex"),
      };
    case "ydd.rebind-shader":
      return {
        type: definition.id,
        drawableIndex: requiredInteger(values, "drawableIndex"),
        modelIndex: requiredInteger(values, "modelIndex"),
        geometryIndex: requiredInteger(values, "geometryIndex"),
        targetShaderIndex: requiredInteger(values, "targetShaderIndex"),
      };
    case "ytd.replace-dds":
    case "ytd.repack-dds":
    case "ytd.repack-png":
      return {
        type: definition.id,
        textureIndex: requiredInteger(values, "textureIndex"),
        replacement: requiredString(values, "replacement"),
      };
    case "ytd.repack-rgba":
      return {
        type: definition.id,
        textureIndex: requiredInteger(values, "textureIndex"),
        width: requiredPositiveInteger(values, "width"),
        height: requiredPositiveInteger(values, "height"),
        replacement: requiredString(values, "replacement"),
      };
    case "ytd.rebuild-compact":
      return { type: definition.id };
    case "ybn.edit-polygon": {
      const radius = optionalPositiveNumber(values, "radius");
      const materialIndex = optionalInteger(values, "materialIndex");
      if (radius === undefined && materialIndex === undefined) {
        throw new Error(
          "YBN polygon edit requires at least radius or materialIndex.",
        );
      }

      return {
        type: definition.id,
        childIndex: requiredInteger(values, "childIndex"),
        polygonIndex: requiredInteger(values, "polygonIndex"),
        kind: requiredString(values, "kind"),
        ...(radius === undefined ? {} : { radius }),
        ...(materialIndex === undefined ? {} : { materialIndex }),
      };
    }
  }
}

function isEditableOperationId(value: string): value is EditableOperationId {
  return Object.prototype.hasOwnProperty.call(FORM_DEFINITIONS, value);
}

function vectorDeltaFields(): OperationFieldDefinition[] {
  return [
    numberField("deltaX", "Delta X"),
    numberField("deltaY", "Delta Y"),
    numberField("deltaZ", "Delta Z"),
  ];
}

function textureBindingFields(
  includeDrawable: boolean,
): OperationFieldDefinition[] {
  return [
    ...(includeDrawable
      ? [integerField("drawableIndex", "Drawable index")]
      : []),
    integerField("sourceShader", "Source shader"),
    integerField("sourceParameter", "Source parameter"),
    integerField("targetShader", "Target shader"),
    integerField("targetParameter", "Target parameter"),
  ];
}

function shaderBindingFields(
  includeDrawable: boolean,
): OperationFieldDefinition[] {
  return [
    ...(includeDrawable
      ? [integerField("drawableIndex", "Drawable index")]
      : []),
    integerField("modelIndex", "Model index"),
    integerField("geometryIndex", "Geometry index"),
    integerField("targetShaderIndex", "Target shader index"),
  ];
}

function integerField(key: string, label: string): OperationFieldDefinition {
  return {
    key,
    label,
    kind: "integer",
    required: true,
    min: 0,
    step: 1,
  };
}

function numberField(key: string, label: string): OperationFieldDefinition {
  return {
    key,
    label,
    kind: "number",
    required: true,
    step: 0.1,
  };
}

function fileField(key: string, label: string): OperationFieldDefinition {
  return {
    key,
    label,
    kind: "file",
    required: true,
  };
}

function parseDelta(values: Record<string, string>): [number, number, number] {
  const delta: [number, number, number] = [
    requiredNumber(values, "deltaX"),
    requiredNumber(values, "deltaY"),
    requiredNumber(values, "deltaZ"),
  ];
  if (delta.every((value) => value === 0)) {
    throw new Error("Translation delta must not be zero.");
  }
  return delta;
}

function requiredString(
  values: Record<string, string>,
  key: string,
): string {
  const value = (values[key] ?? "").trim();
  if (!value) {
    throw new Error(key + " is required.");
  }
  return value;
}

function requiredNumber(
  values: Record<string, string>,
  key: string,
): number {
  const raw = requiredString(values, key);
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    throw new Error(key + " must be a finite number.");
  }
  return value;
}

function requiredInteger(
  values: Record<string, string>,
  key: string,
): number {
  const value = requiredNumber(values, key);
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(key + " must be a non-negative integer.");
  }
  return value;
}

function requiredPositiveInteger(
  values: Record<string, string>,
  key: string,
): number {
  const value = requiredInteger(values, key);
  if (value === 0) {
    throw new Error(key + " must be greater than zero.");
  }
  return value;
}

function optionalInteger(
  values: Record<string, string>,
  key: string,
): number | undefined {
  const raw = (values[key] ?? "").trim();
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(key + " must be a non-negative integer.");
  }
  return value;
}

function optionalPositiveNumber(
  values: Record<string, string>,
  key: string,
): number | undefined {
  const raw = (values[key] ?? "").trim();
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(key + " must be a finite number greater than zero.");
  }
  return value;
}
