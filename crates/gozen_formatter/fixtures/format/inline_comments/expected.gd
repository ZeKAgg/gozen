extends Node

func custom_round(value: float) -> int:
	if value >= 1.5:
		return ceil(value)  # Round up
	else:
		return floor(value)  # Round down

func example():
	var skill = Skill.new()  # Create a new instance
	skill.id = 5
	return skill  # Default fallback if no match is found
