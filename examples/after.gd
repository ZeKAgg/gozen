# after.gd — The same file after running `gozen check --write`.
# Issues that gozen auto-fixed are resolved. Remaining issues are
# flagged as diagnostics for the developer to address manually.

extends CharacterBody2D

# Fixed: SCREAMING_SNAKE_CASE (manual fix prompted by gozen)
const PLAYER_SPEED = 200.0

# Fixed: unused variable removed (auto-fix)

# Addressed: type hint added (manual fix)
var health: int = 100

# Fixed: boolean operators — no change needed here, but `== true` below is gone
var is_alive: bool = true

# Fixed: `export` -> `@export` (auto-fix by noDeprecatedSyntax rule)
@export var damage: int = 10

signal health_changed(new_health: int)

# Fixed: unused signal removed (manual fix prompted by gozen)

func _ready():
	print("Player ready")

func _process(delta: float) -> void:
	# Fixed: `is_alive == true` -> `is_alive` (auto-fix by noBoolComparison)
	if is_alive:
		# Fixed: self-assignment removed (manual fix)
		move(delta)

	# Fixed: self-comparison removed (manual fix)

func move(delta: float) -> void:
	var direction := Input.get_axis("move_left", "move_right")
	# Fixed: `&&` -> `and` (auto-fix by booleanOperators rule)
	if direction != 0 and is_alive:
		velocity.x = direction * PLAYER_SPEED
	else:
		velocity.x = 0
	move_and_slide()

func take_damage(amount: int) -> void:
	health -= amount
	health_changed.emit(health)
	if health <= 0:
		is_alive = false
		queue_free()
