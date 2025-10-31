# Ivy Guide Update Plan

## Objective
Update the ivy-guide (mdBook documentation) to incorporate new comprehensive documentation from Structure.md and README.md, ensuring consistency, accuracy, and improved user experience.

## Current State Assessment
- ivy-guide uses mdBook with book.toml configuration.
- Existing content in src/ includes fundamentals/ with various .md files.
- New docs provide detailed crate breakdowns, architecture, working principles, and features.

## Plan Steps

### 1. Review Existing Guide Structure
- Read all current .md files in ivy-guide/src/ to understand current content and organization.
- Identify overlapping or outdated sections (e.g., architecture, getting started).
- Note any missing topics from new docs (e.g., Templates system, detailed crate info).

### 2. Identify Update Targets
- **Introduction/Overview**: Update with new README overview, key features, and working principles.
- **Architecture Section**: Replace or enhance with Structure.md architecture details, including App/Layers, ECS, asset system, rendering pipeline.
- **Crate Documentation**: Add detailed pages for each major crate (ivy-core, ivy-wgpu, ivy-physics, etc.) based on Structure.md breakdowns.
- **Working Principles**: Expand with in-depth explanations from README (ECS, asset management, physics, input, UI, Templates).
- **Getting Started**: Update with new installation, quick start, and examples from README.
- **Features and Functionality**: Add sections on rendering, physics, input, UI, asset pipeline, utilities.
- **Editor and Tools**: New section on integrated editor, gizmos, property editing, template editing.
- **Gallery/Examples**: Incorporate or reference gallery images and example descriptions.

### 3. Content Integration
- Extract relevant sections from README.md and Structure.md.
- Adapt content for guide format (remove badges, adjust links, add cross-references).
- Ensure consistent terminology and structure across files.
- Add internal links between sections (e.g., link to crate docs from architecture).

### 4. File Organization
- Create new .md files for major sections (e.g., crates/, features/, editor/).
- Update SUMMARY.md to reflect new structure and include all pages.
- Maintain logical flow: Overview → Getting Started → Architecture → Features → Crates → Editor → Contributing.

### 5. Enhancements
- Add code examples and snippets where appropriate.
- Include diagrams or flowcharts for complex systems (e.g., rendering pipeline).
- Ensure mobile-friendly formatting and clear headings.

### 6. Validation and Testing
- Run `mdbook build` to check for errors and broken links.
- Review generated HTML for readability and navigation.
- Cross-check with codebase for accuracy (spot-check key files).
- Test links to external resources (API docs, GitHub).

### 7. Finalization
- Update book.toml if needed (e.g., title, description).
- Add any missing metadata or frontmatter.
- Commit changes with descriptive message.

## Timeline
- Assessment: 1-2 hours
- Content Updates: 4-6 hours
- Organization and Links: 2-3 hours
- Testing and Validation: 1-2 hours

## Dependencies
- Access to Structure.md and README.md for content extraction.
- mdBook installed for testing builds.
- Familiarity with Ivy codebase for validation.

## Success Criteria
- Guide builds without errors.
- All major topics from new docs are covered.
- Content is accurate, up-to-date, and user-friendly.
- Navigation is intuitive with proper cross-linking.