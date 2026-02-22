extends Node

func convert():
	var location = Location.new()
	location.id = dict.get("id", 0)
	location.title = dict.get("title", "")

	var enemy_dict = dict.get("enemy", null)
	location.enemy = Location.Enemy.new()
	location.enemy.name = enemy_dict.get("name", "")

	var reward_dict = dict.get("reward", null)
	location.reward = Location.Reward.new()
	location.reward.experience = reward_dict.get("experience", 0)

	return location
