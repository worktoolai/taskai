# 목적
cli를 활용한 task 관리 도구

# 사용 기술
- sqlite
- rust

# 컨셉
ai agent가 plan후 task를 만든다.
task를 cli를 통해 저장하고 task는 이후 해야할 task에 대해 응답을 준다
태스크간의 순서를 정의할수 있어야하고
병렬을 고려해서 태스크간 관계 그래프도 가능해야한다
전체 plan과 특정 태스크를 수행할때 필요한 설계 문서도 같이 제공할 수 있다.
차후 history관리도 가능하다.

# 요구사항
저장은 .git root를 기준으로 .worktoolai/taskai 하위에 저장한다.
