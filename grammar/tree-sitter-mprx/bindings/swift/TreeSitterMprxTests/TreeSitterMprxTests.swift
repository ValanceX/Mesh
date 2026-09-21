import XCTest
import SwiftTreeSitter
import TreeSitterMprx

final class TreeSitterMprxTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_mprx())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Mprx grammar")
    }
}
