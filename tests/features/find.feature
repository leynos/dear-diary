Feature: Finding information in Qdrant

  Scenario: Search returns matching results
    Given a Qdrant connector with stored entries
    When I search for "milk" in collection "memories"
    Then the search should return matching entries

  Scenario: Search in empty collection returns no results
    Given a Qdrant connector with no stored entries
    When I search for "anything" in collection "empty"
    Then the search should return no results

  Scenario: Search without specifying collection uses default
    Given a Qdrant connector with default collection "default_memories"
    When I search for "reminder" without specifying a collection
    Then the search should use collection "default_memories"
