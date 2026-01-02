Feature: Storing information in Qdrant

  Scenario: Store a simple memory
    Given a configured Qdrant connector
    When I store "Remember to buy milk" in collection "memories"
    Then the store operation should succeed

  Scenario: Store information with metadata
    Given a configured Qdrant connector
    When I store "Meeting notes from today" with metadata in collection "work"
    Then the store operation should succeed
    And the entry should have the specified metadata

  Scenario: Store without specifying collection uses default
    Given a configured Qdrant connector with default collection "default_memories"
    When I store "Important reminder" without specifying a collection
    Then the store operation should use collection "default_memories"
