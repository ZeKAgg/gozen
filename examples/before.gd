# before.gd — A GDScript file with several issues that gozen catches.
# Run: gozen check --write examples/before.gd
# Then compare with after.gd to see what changed.

extends CharacterBody2D

# Style: naming convention — should be SCREAMING_SNAKE_CASE
const player_speed = 200.0

# Correctness: unused variable
var unused_counter: int = 0

# Style: missing type hint (opt-in rule: noUntypedDeclaration)
var health = 100

# Style: boolean comparison — `if flag == true` should be `if flag`
var is_alive: bool = true

# Correctness: deprecated syntax — `export` should be `@export`
export var damage: int = 10

signal health_changed(new_health: int)

# Correctness: unused signal
signal score_updated

func _ready():
	# Performance: expensive call in _process would be caught there
	print("Player ready")

func _process(delta):
	# Style: boolean comparison
	if is_alive == true:
		# Correctness: self-assignment
		health = health
		move(delta)

	# Suspicious: self-comparison (always true)
	if health >= health:
		pass

func move(delta):
	# Correctness: unused parameter — `delta` not used in a meaningful way
	var direction = Input.get_axis("move_left", "move_right")
	# Style: should use `and` / `or` instead of `&&` / `||`
	if direction != 0 && is_alive:
		velocity.x = direction * player_speed
	else:
		velocity.x = 0
	move_and_slide()

func take_damage(amount: int):
	health -= amount
	health_changed.emit(health)
	if health <= 0:
		is_alive = false
		# Correctness: deprecated API — queue_free() is fine,
		# but referencing the node after free is a problem
		queue_free()
