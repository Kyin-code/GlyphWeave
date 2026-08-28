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
	_load_visible_chunks()

func _process(_delta: float) -> void:
	if camera == null: return
	var target := Vector3(camera.position.x, 0, camera.position.z)
	var cx := floori(target.x / CHUNK_SIZE)
	var cz := floori(target.z / CHUNK_SIZE)
	for chunk in scene_index.get("chunks", []):
		var key := "%d,%d" % [chunk.chunkX, chunk.chunkZ]
		var near := abs(int(chunk.chunkX) - cx) <= 1 and abs(int(chunk.chunkZ) - cz) <= 1
		if near and not loaded.has(key): _load_chunk(chunk, key)
		if not near and loaded.has(key): loaded[key].queue_free(); loaded.erase(key)
	status_label.text = "scene=%s  chunk=%d,%d  loaded=%d  X=%d Z=%d Y=%.2f" % [scene_index.sceneId, cx, cz, loaded.size(), int(target.x), int(target.z), target.y]

func _load_visible_chunks() -> void:
	for chunk in scene_index.get("chunks", []): _load_chunk(chunk, "%d,%d" % [chunk.chunkX, chunk.chunkZ])

func _load_chunk(chunk: Dictionary, key: String) -> void:
	var node := Node3D.new(); node.name = "StreamingChunk_%s" % key; add_child(node); loaded[key] = node
	var mesh_instance := MeshInstance3D.new(); var plane := PlaneMesh.new(); plane.size = Vector2(chunk.validWidthM, chunk.validDepthM); plane.subdivide_width = 32; plane.subdivide_depth = 32
	var material := StandardMaterial3D.new(); material.albedo_color = Color("#667d58"); plane.material = material; mesh_instance.mesh = plane; mesh_instance.position = Vector3(chunk.worldX + chunk.validWidthM / 2.0, 0, chunk.worldZ + chunk.validDepthM / 2.0); node.add_child(mesh_instance)

func _read_json(path: String) -> Dictionary:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null: return {}
	var value = JSON.parse_string(file.get_as_text())
	return value if value is Dictionary else {}

func _make_label(text: String) -> Label:
	var label := Label.new(); label.text = text; label.position = Vector2(16, 16); label.add_theme_color_override("font_color", Color("#d7dfd8")); add_child(label); return label
