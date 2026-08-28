extends Node3D

const CHUNK_SIZE := 512
var world: Dictionary
var scene_index: Dictionary
var loaded: Dictionary = {}
var camera: Camera3D
var status_label: Label

func _ready() -> void:
	world = _read_json("res://../world.json")
	if world.is_empty():
		status_label = _make_label("GlyphWeave: world.json missing")
		return
	camera = $Camera3D
	var scene_path: String = world.get("scenes", [""])[0]
	scene_index = _read_json("res://../" + scene_path)
	status_label = _make_label("scene=%s  chunks=0  coordinates: X/Z/Y" % scene_index.get("sceneId", "unknown"))
	_update_visible_chunks()

func _process(_delta: float) -> void:
	if camera == null: return
	var target := Vector3(camera.position.x, 0, camera.position.z)
	var cx := floori(target.x / CHUNK_SIZE)
	var cz := floori(target.z / CHUNK_SIZE)
	for chunk in scene_index.get("chunks", []):
		var key := "%d,%d" % [chunk.chunkX, chunk.chunkZ]
		var near: bool = abs(int(chunk.chunkX) - cx) <= 1 and abs(int(chunk.chunkZ) - cz) <= 1
		if near and not loaded.has(key): _load_chunk(chunk, key)
		if not near and loaded.has(key): loaded[key].queue_free(); loaded.erase(key)
	status_label.text = "scene=%s  chunk=%d,%d  loaded=%d  X=%d Z=%d Y=%.2f" % [scene_index.sceneId, cx, cz, loaded.size(), int(target.x), int(target.z), target.y]

func _update_visible_chunks() -> void:
	var target := Vector3(camera.position.x, 0, camera.position.z)
	var cx := floori(target.x / CHUNK_SIZE)
	var cz := floori(target.z / CHUNK_SIZE)
	for chunk in scene_index.get("chunks", []):
		var key := "%d,%d" % [chunk.chunkX, chunk.chunkZ]
		if abs(int(chunk.chunkX) - cx) <= 1 and abs(int(chunk.chunkZ) - cz) <= 1 and not loaded.has(key): _load_chunk(chunk, key)

func _load_chunk(chunk: Dictionary, key: String) -> void:
	var node := Node3D.new(); node.name = "StreamingChunk_%s" % key; add_child(node); loaded[key] = node
	var mesh_instance := MeshInstance3D.new()
	mesh_instance.mesh = _load_height_mesh(chunk)
	mesh_instance.position = Vector3(chunk.worldX, 0, chunk.worldZ)
	node.add_child(mesh_instance)

func _load_height_mesh(chunk: Dictionary) -> ArrayMesh:
	var file := FileAccess.open("res://../scenes/%s/%s" % [scene_index.sceneId, chunk.heightFile], FileAccess.READ)
	var width: int = chunk.validWidthM
	var depth: int = chunk.validDepthM
	var step := 16
	var cols := ceili(float(width) / step) + 1
	var rows := ceili(float(depth) / step) + 1
	var vertices := PackedVector3Array()
	var indices := PackedInt32Array()
	for z in range(rows):
		for x in range(cols):
			var sx := mini(x * step, width - 1)
			var sz := mini(z * step, depth - 1)
			file.seek((sz * width + sx) * 2)
			var height := float(file.get_16()) / 4.0
			vertices.append(Vector3(sx, height, sz))
	for z in range(rows - 1):
		for x in range(cols - 1):
			var a := z * cols + x
			indices.append_array(PackedInt32Array([a, a + 1, a + cols, a + 1, a + cols + 1, a + cols]))
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var material := StandardMaterial3D.new(); material.albedo_color = Color("#667d58"); mesh.surface_set_material(0, material)
	return mesh

func _read_json(path: String) -> Dictionary:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null: return {}
	var value = JSON.parse_string(file.get_as_text())
	return value if value is Dictionary else {}

func _make_label(text: String) -> Label:
	var label := Label.new(); label.text = text; label.position = Vector2(16, 16); label.add_theme_color_override("font_color", Color("#d7dfd8")); add_child(label); return label
